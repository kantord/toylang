use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::ast::{BinOp, Def, Expr, File, TypeExpr};
use crate::error::Error;
use crate::tir::{self, Kind, LocalId, Tir};
use crate::ty::{Sig, Type};

struct Ctx<'a> {
    sigs: &'a HashMap<String, Sig>,
    /// Named bindings. At most one, since functions are unary and there is no `let`.
    scope: Vec<(String, Type)>,
    /// What `.` refers to here, if anything: its type and the local holding it.
    subject: Option<(Type, LocalId)>,
    /// The type `input` was checked against, filled in the first time it is used.
    input: &'a RefCell<Option<Type>>,
    next_local: &'a Cell<LocalId>,
}

impl Ctx<'_> {
    fn with(&self, subject: Option<(Type, LocalId)>) -> Ctx<'_> {
        Ctx {
            sigs: self.sigs,
            scope: self.scope.clone(),
            subject,
            input: self.input,
            next_local: self.next_local,
        }
    }

    fn fresh(&self) -> LocalId {
        let id = self.next_local.get();
        self.next_local.set(id + 1);
        id
    }
}

pub fn check(file: &File) -> Result<tir::Program, Error> {
    let sigs = signatures(&file.defs)?;
    let input = RefCell::new(None);
    let next_local = Cell::new(0);

    // Signatures are collected before any body is checked, so a definition may call one that
    // appears later in the file. This is also what recursion will need.
    let mut funcs = Vec::new();
    for def in &file.defs {
        let sig = &sigs[&def.name];
        let ctx = Ctx {
            sigs: &sigs,
            scope: vec![(def.param.name.clone(), sig.param.clone())],
            subject: None,
            input: &input,
            next_local: &next_local,
        };
        let body = synth(&ctx, &def.body)?;
        if body.ty != sig.ret {
            return Err(Error::new(
                def.body.span(),
                format!(
                    "`{}` declares it returns {}, but its body is {}",
                    def.name, sig.ret, body.ty
                ),
            ));
        }
        funcs.push(tir::Func {
            name: def.name.clone(),
            param: def.param.name.clone(),
            param_ty: sig.param.clone(),
            body,
        });
    }

    let ctx = Ctx {
        sigs: &sigs,
        scope: Vec::new(),
        subject: None,
        input: &input,
        next_local: &next_local,
    };
    let body = synth(&ctx, &file.body)?;
    Ok(tir::Program { funcs, body, input: input.into_inner() })
}

/// Functions the language provides. Unary like every other function, so they need no special
/// call syntax and are looked up before user definitions.
fn builtin(name: &str) -> Option<(tir::Builtin, Sig)> {
    let vec_of = |t: Type| Type::Vec(Box::new(t));
    Some(match name {
        "str" => (tir::Builtin::IntToStr, Sig { param: Type::Int, ret: Type::Str }),
        "range" => (tir::Builtin::Range, Sig { param: Type::Int, ret: vec_of(Type::Int) }),
        "unlines" => (tir::Builtin::Unlines, Sig { param: vec_of(Type::Str), ret: Type::Str }),
        _ => return None,
    })
}

fn signatures(defs: &[Def]) -> Result<HashMap<String, Sig>, Error> {
    let mut sigs = HashMap::new();
    for def in defs {
        if builtin(&def.name).is_some() {
            return Err(Error::new(
                def.span,
                format!("`{}` is a builtin and cannot be redefined", def.name),
            ));
        }
        if sigs.contains_key(&def.name) {
            return Err(Error::new(def.span, format!("`{}` is defined twice", def.name)));
        }
        let sig = Sig { param: resolve(&def.param.ty)?, ret: resolve(&def.ret)? };
        sigs.insert(def.name.clone(), sig);
    }
    Ok(sigs)
}

fn resolve(ty: &TypeExpr) -> Result<Type, Error> {
    match ty {
        TypeExpr::Named { name, span } => {
            Type::from_name(name).ok_or_else(|| Error::new(*span, format!("unknown type `{name}`")))
        }
        TypeExpr::Vec { elem, .. } => Ok(Type::Vec(Box::new(resolve(elem)?))),
        TypeExpr::Record { fields, span } => {
            let mut out = Vec::new();
            for (name, ty) in fields {
                if out.iter().any(|(n, _): &(String, Type)| n == name) {
                    return Err(Error::new(*span, format!("field `{name}` is declared twice")));
                }
                out.push((name.clone(), resolve(ty)?));
            }
            Ok(Type::record(out))
        }
    }
}

fn synth(ctx: &Ctx, expr: &Expr) -> Result<Tir, Error> {
    match expr {
        Expr::Str { text, .. } => Ok(Tir::new(Type::Str, Kind::Str(text.clone()))),
        Expr::Int { value, span } => {
            // The literal is the one place a value could enter without meeting the 32-bit rule,
            // and four backends agreed on the wrong answer only because each held it in its own
            // wider representation until an operator wrapped it. Go refuses to compile such a
            // constant at all, which is what made the hole visible.
            if *value > i32::MAX as i64 {
                return Err(Error::new(
                    *span,
                    format!("integer `{value}` does not fit in Int, which is 32 bits"),
                ));
            }
            Ok(Tir::new(Type::Int, Kind::Int(*value)))
        }

        Expr::Subject { span } => match &ctx.subject {
            Some((ty, id)) => Ok(Tir::new(ty.clone(), Kind::Local(*id))),
            None => Err(Error::new(*span, "`.` is not bound here")),
        },

        Expr::Var { name, span } => ctx
            .scope
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| Tir::new(t.clone(), Kind::Var(name.clone())))
            .ok_or_else(|| Error::new(*span, format!("`{name}` is not defined"))),

        Expr::ProductLit { components, .. } => {
            let mut built: Vec<(String, Tir)> = Vec::new();
            for (name, name_span, value) in components {
                if built.iter().any(|(seen, _)| seen == name) {
                    return Err(Error::new(
                        *name_span,
                        format!("component `{name}` is given twice"),
                    ));
                }
                built.push((name.clone(), synth(ctx, value)?));
            }
            // Sorted to match Type::record, so a component's index is the same in the value and
            // in the type.
            built.sort_by(|a, b| a.0.cmp(&b.0));
            let ty =
                Type::record(built.iter().map(|(n, t)| (n.clone(), t.ty.clone())).collect());
            Ok(Tir::new(ty, Kind::ProductLit { components: built }))
        }

        Expr::VecLit { items, span } => {
            let Some(first) = items.first() else {
                // Nothing says what an empty literal contains, and there is no expected type to
                // supply it. Guessing here is what the annotation rule exists to avoid.
                return Err(Error::new(*span, "cannot tell what `[]` contains"));
            };
            let head = synth(ctx, first)?;
            let elem = head.ty.clone();
            let mut out = vec![head];
            for item in &items[1..] {
                out.push(expect(ctx, item, &elem)?);
            }
            Ok(Tir::new(Type::Vec(Box::new(elem)), Kind::VecLit(out)))
        }

        // `|` binds `.` in the right side to the value of the left. It is composition, not a
        // map: the operators that distribute over a Vec do so themselves.
        Expr::Pipe { lhs, rhs, .. } => {
            let value = synth(ctx, lhs)?;
            let local = ctx.fresh();
            let body = synth(&ctx.with(Some((value.ty.clone(), local))), rhs)?;
            Ok(Tir::new(
                body.ty.clone(),
                Kind::Bind { local, value: Box::new(value), body: Box::new(body) },
            ))
        }

        // A mask over the subject Vec. The predicate is checked with `.` rebound to the element
        // type rather than evaluated in the enclosing scope.
        Expr::Select { pred, span } => {
            let Some((subject, id)) = ctx.subject.clone() else {
                return Err(Error::new(*span, "`select` needs a subject, so it must follow `|`"));
            };
            let Some(elem) = subject.elem().cloned() else {
                return Err(Error::new(*span, format!("`select` needs a Vec, found {subject}")));
            };
            let param = ctx.fresh();
            let pred = expect(&ctx.with(Some((elem, param))), pred, &Type::Bool)?;
            let source = Tir::new(subject.clone(), Kind::Local(id));
            Ok(Tir::new(
                subject,
                Kind::Select { source: Box::new(source), param, pred: Box::new(pred) },
            ))
        }

        // The one way to produce a new element value. `select` removes elements and a field
        // access reads a component; neither can turn a Vec<Int> into a Vec<Str>.
        Expr::Map { body, span } => {
            let Some((subject, id)) = ctx.subject.clone() else {
                return Err(Error::new(*span, "`map` needs a subject, so it must follow `|`"));
            };
            let Some(elem) = subject.elem().cloned() else {
                return Err(Error::new(*span, format!("`map` needs a Vec, found {subject}")));
            };
            let param = ctx.fresh();
            let body = synth(&ctx.with(Some((elem, param))), body)?;
            let source = Tir::new(subject, Kind::Local(id));
            Ok(Tir::new(
                Type::Vec(Box::new(body.ty.clone())),
                Kind::Map { source: Box::new(source), param, body: Box::new(body) },
            ))
        }

        Expr::Field { .. } | Expr::Index { .. } | Expr::Unwrap { .. } => {
            access(ctx, expr).map(|(tir, _, _)| tir)
        }

        // A spec that specs nothing. `[]` says what happens to a dimension, so with no access
        // after it there is no dimension being reached into and nothing for it to say.
        Expr::Project { span, .. } => {
            Err(Error::new(*span, "`[]` must be followed by a field access"))
        }

        // `input` is only ever checked, never synthesised, for the same reason a lambda is:
        // nothing here says what it contains, and guessing is what the annotation rule avoids.
        Expr::Input { span } => Err(Error::new(*span, "cannot tell what `input` contains")),

        Expr::Call { func, func_span, arg, .. } => {
            if let Some((which, sig)) = builtin(func) {
                let arg = expect(ctx, arg, &sig.param)?;
                return Ok(Tir::new(sig.ret, Kind::Builtin { which, arg: Box::new(arg) }));
            }
            let sig = ctx
                .sigs
                .get(func)
                .ok_or_else(|| Error::new(*func_span, format!("`{func}` is not a function")))?;
            let arg = expect(ctx, arg, &sig.param)?;
            Ok(Tir::new(sig.ret.clone(), Kind::Call { func: func.clone(), arg: Box::new(arg) }))
        }

        Expr::Neg { base, span } => {
            // A minus directly on a literal is part of the literal, so the most negative Int can
            // be written even though its magnitude is one past the most positive. This is the
            // rule Rust uses, and it is why `-` was not folded into the lexer: `a -1` has to
            // stay `a - 1`.
            if let Expr::Int { value, span: lit } = base.as_ref() {
                if *value > -(i32::MIN as i64) {
                    return Err(Error::new(
                        *lit,
                        format!("integer `-{value}` does not fit in Int, which is 32 bits"),
                    ));
                }
                return Ok(Tir::new(Type::Int, Kind::Int(-value)));
            }
            let inner = expect(ctx, base, &Type::Int)?;
            let zero = Tir::new(Type::Int, Kind::Int(0));
            let _ = span;
            Ok(Tir::new(
                Type::Int,
                Kind::Arith { op: BinOp::Sub, lhs: Box::new(zero), rhs: Box::new(inner) },
            ))
        }

        // The first construct that consumes a type rather than carrying one: the condition has
        // to be exactly one Bool, and both branches have to agree.
        Expr::Cond { then, cond, otherwise, .. } => {
            let cond = expect(ctx, cond, &Type::Bool)?;
            let then = synth(ctx, then)?;
            let otherwise = expect(ctx, otherwise, &then.ty)?;
            Ok(Tir::new(
                then.ty.clone(),
                Kind::Cond {
                    cond: Box::new(cond),
                    then: Box::new(then),
                    otherwise: Box::new(otherwise),
                },
            ))
        }

        Expr::Binary { op, lhs, rhs, .. } => binary(ctx, *op, lhs, rhs),
    }
}

/// Walk an access chain left to right, carrying what we are currently looking at and how many
/// dimensions we are inside.
///
/// Every dimension needs a spec. `[]` enters one, so it strips a layer off what we are looking at
/// and adds one to the depth; a field access reads a component of it and leaves the depth alone.
/// The expression's type is what we are looking at, wrapped back up that many times.
///
/// This is why `db.users.name` is an error and `db.users[].name` is not: the first never said
/// what happens to the dimension it reached through.
fn access(ctx: &Ctx, expr: &Expr) -> Result<(Tir, Type, usize), Error> {
    match expr {
        Expr::Project { base, span } => {
            let (tir, elem, depth) = access(ctx, base)?;
            let Some(inner) = elem.elem().cloned() else {
                return Err(Error::new(*span, format!("`[]` needs a dimension, found {elem}")));
            };
            Ok((tir, inner, depth + 1))
        }

        // The absence stops being carried and starts being asserted.
        Expr::Unwrap { base, span } => {
            let (base_tir, elem, depth) = access(ctx, base)?;
            let Type::Opt(inner) = elem else {
                return Err(Error::new(*span, format!("`!` needs an Opt, found {elem}")));
            };
            let inner = *inner;
            let mut ty = inner.clone();
            for _ in 0..depth {
                ty = Type::Vec(Box::new(ty));
            }
            let tir = Tir::new(ty, Kind::Unwrap { base: Box::new(base_tir) });
            Ok((tir, inner, depth))
        }

        // Collapsing a dimension. The entry may not be there, so what comes out is `Opt`.
        Expr::Index { base, index, span } => {
            let (base_tir, elem, depth) = access(ctx, base)?;
            let Some(inner) = elem.elem().cloned() else {
                return Err(Error::new(*span, format!("`[i]` needs a dimension, found {elem}")));
            };
            let index_tir = expect(ctx, index, &Type::Int)?;
            let elem_is_record = matches!(inner, Type::Record(_));
            let out = Type::Opt(Box::new(inner));
            let mut ty = out.clone();
            for _ in 0..depth {
                ty = Type::Vec(Box::new(ty));
            }
            let tir = Tir::new(
                ty,
                Kind::Index {
                    base: Box::new(base_tir),
                    index: Box::new(index_tir),
                    depth,
                    elem_is_record,
                },
            );
            Ok((tir, out, depth))
        }

        Expr::Field { base, name, span } => {
            let (base_tir, elem, depth) = access(ctx, base)?;
            if elem.elem().is_some() {
                return Err(Error::new(
                    *span,
                    format!(
                        "`.{name}` needs a record, found {elem}: give the dimension a spec with `[]`"
                    ),
                ));
            }
            let Some(field) = elem.field(name) else {
                return Err(Error::new(*span, format!("no field `{name}` on {elem}")));
            };
            let field = field.clone();
            let mut ty = field.clone();
            for _ in 0..depth {
                ty = Type::Vec(Box::new(ty));
            }
            let tir = Tir::new(ty, Kind::Field { base: Box::new(base_tir), name: name.clone() });
            Ok((tir, field, depth))
        }

        other => {
            let tir = synth(ctx, other)?;
            let ty = tir.ty.clone();
            Ok((tir, ty, 0))
        }
    }
}

fn binary(ctx: &Ctx, op: BinOp, lhs: &Expr, rhs: &Expr) -> Result<Tir, Error> {
    let left = synth(ctx, lhs)?;

    // Q2 is open, so an operator over a Vec is rejected rather than being silently given
    // broadcast or zip semantics. Under C1 that restriction is ordinary typing: there is no
    // separate cardinality to check, because a Vec is just a type.
    if left.ty.elem().is_some() {
        return Err(Error::new(lhs.span(), format!("`{op}` does not apply to {}", left.ty)));
    }

    if op.is_comparison() {
        let right = expect(ctx, rhs, &left.ty)?;
        return Ok(Tir::new(
            Type::Bool,
            Kind::Compare { op, lhs: Box::new(left), rhs: Box::new(right) },
        ));
    }

    if op.is_arithmetic() {
        if left.ty != Type::Int {
            return Err(Error::new(lhs.span(), format!("expected Int, found {}", left.ty)));
        }
        let right = expect(ctx, rhs, &Type::Int)?;
        return Ok(Tir::new(
            Type::Int,
            Kind::Arith { op, lhs: Box::new(left), rhs: Box::new(right) },
        ));
    }

    // `+` is the one operator whose meaning depends on its operands: addition on Int and
    // concatenation on Str. Both sides must agree, since nothing is coerced.
    match left.ty {
        Type::Int => {
            let right = expect(ctx, rhs, &Type::Int)?;
            Ok(Tir::new(
                Type::Int,
                Kind::Arith { op: BinOp::Add, lhs: Box::new(left), rhs: Box::new(right) },
            ))
        }
        Type::Str => {
            let right = expect(ctx, rhs, &Type::Str)?;
            Ok(Tir::new(Type::Str, Kind::Concat(Box::new(left), Box::new(right))))
        }
        other => Err(Error::new(lhs.span(), format!("`+` needs Int or Str, found {other}"))),
    }
}

/// The checking direction: an expected type goes in, and the expression is verified against it
/// rather than asked what it is. Most forms answer both questions, but not all do.
fn expect(ctx: &Ctx, expr: &Expr, want: &Type) -> Result<Tir, Error> {
    // The forms whose type comes from their position rather than their contents.
    if let Expr::Input { span } = expr {
        let mut slot = ctx.input.borrow_mut();
        match slot.as_ref() {
            None => *slot = Some(want.clone()),
            Some(prev) if prev != want => {
                return Err(Error::new(
                    *span,
                    format!("`input` is used as {prev} here and as {want} elsewhere"),
                ));
            }
            Some(_) => {}
        }
        return Ok(Tir::new(want.clone(), Kind::Input));
    }

    let found = synth(ctx, expr)?;
    if &found.ty != want {
        return Err(Error::new(expr.span(), format!("expected {want}, found {}", found.ty)));
    }
    Ok(found)
}
