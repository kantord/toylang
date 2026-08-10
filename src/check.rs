use std::cell::RefCell;
use std::collections::HashMap;

use crate::ast::{BinOp, Def, Expr, File, Span, TypeExpr};
use crate::error::Error;
use crate::ty::{Sig, Type};

struct Ctx<'a> {
    sigs: &'a HashMap<String, Sig>,
    /// Named bindings. At most one, since functions are unary and there is no `let`.
    scope: Vec<(String, Type)>,
    /// What `.` refers to here, if anything.
    subject: Option<Type>,
    /// The type `input` was checked against, filled in the first time it is used.
    input: &'a RefCell<Option<Type>>,
    /// How many Vec layers each field access distributes over. The checker knows this and the
    /// runtime must not have to rediscover it, so it is recorded here for lowering.
    depths: &'a RefCell<HashMap<Span, usize>>,
}

impl Ctx<'_> {
    fn with(&self, subject: Option<Type>) -> Ctx<'_> {
        Ctx {
            sigs: self.sigs,
            scope: self.scope.clone(),
            subject,
            input: self.input,
            depths: self.depths,
        }
    }
}

/// What a program needs and what it produces.
pub struct Checked {
    pub ty: Type,
    pub input: Option<Type>,
    pub field_depths: HashMap<Span, usize>,
}

pub fn check(file: &File) -> Result<Checked, Error> {
    let sigs = signatures(&file.defs)?;
    let input = RefCell::new(None);
    let depths = RefCell::new(HashMap::new());

    // Signatures are collected before any body is checked, so a definition may call one that
    // appears later in the file. This is also what recursion will need.
    for def in &file.defs {
        let sig = &sigs[&def.name];
        let ctx = Ctx {
            sigs: &sigs,
            scope: vec![(def.param.name.clone(), sig.param.clone())],
            subject: None,
            input: &input,
            depths: &depths,
        };
        let found = synth(&ctx, &def.body)?;
        if found != sig.ret {
            return Err(Error::new(
                def.body.span(),
                format!("`{}` declares it returns {}, but its body is {found}", def.name, sig.ret),
            ));
        }
    }

    let ctx =
        Ctx { sigs: &sigs, scope: Vec::new(), subject: None, input: &input, depths: &depths };
    let ty = synth(&ctx, &file.body)?;
    Ok(Checked { ty, input: input.into_inner(), field_depths: depths.into_inner() })
}

fn signatures(defs: &[Def]) -> Result<HashMap<String, Sig>, Error> {
    let mut sigs = HashMap::new();
    for def in defs {
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
        TypeExpr::Named { name, span } => Type::from_name(name)
            .ok_or_else(|| Error::new(*span, format!("unknown type `{name}`"))),
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

fn synth(ctx: &Ctx, expr: &Expr) -> Result<Type, Error> {
    match expr {
        Expr::Str { .. } => Ok(Type::Str),
        Expr::Int { .. } => Ok(Type::Int),

        Expr::Subject { span } => ctx
            .subject
            .clone()
            .ok_or_else(|| Error::new(*span, "`.` is not bound here")),

        Expr::Var { name, span } => ctx
            .scope
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| t.clone())
            .ok_or_else(|| Error::new(*span, format!("`{name}` is not defined"))),

        Expr::VecLit { items, span } => {
            let Some(first) = items.first() else {
                // Nothing says what an empty literal contains, and there is no expected type to
                // supply it. Guessing here is what the annotation rule exists to avoid.
                return Err(Error::new(*span, "cannot tell what `[]` contains"));
            };
            let elem = synth(ctx, first)?;
            for item in &items[1..] {
                expect(ctx, item, &elem)?;
            }
            Ok(Type::Vec(Box::new(elem)))
        }

        // Projection by every index. On a Vec that is the same extent, so this is the identity;
        // see research-log/a-pure-value-layer-dissolves-jqs-iteration-operators.md.
        Expr::Project { base, span } => {
            let base_ty = synth(ctx, base)?;
            if base_ty.elem().is_none() {
                return Err(Error::new(*span, format!("`[]` needs a Vec, found {base_ty}")));
            }
            Ok(base_ty)
        }

        // `|` binds `.` in the right side to the value of the left. It is composition, not a
        // map: the operators that distribute over a Vec do so themselves.
        Expr::Pipe { lhs, rhs, .. } => {
            let subject = synth(ctx, lhs)?;
            synth(&ctx.with(Some(subject)), rhs)
        }

        // Field access distributes over a Vec rather than needing a map, which is how `.name`
        // reads an element field straight off a filtered collection.
        Expr::Field { base, name, span } => {
            let base_ty = synth(ctx, base)?;
            let mut depth = 0;
            let mut inner = &base_ty;
            while let Some(elem) = inner.elem() {
                depth += 1;
                inner = elem;
            }
            let Some(field) = inner.field(name) else {
                return Err(Error::new(*span, format!("no field `{name}` on {inner}")));
            };
            let mut out = field.clone();
            for _ in 0..depth {
                out = Type::Vec(Box::new(out));
            }
            ctx.depths.borrow_mut().insert(*span, depth);
            Ok(out)
        }

        // `input` is only ever checked, never synthesised, for the same reason a lambda is:
        // nothing here says what it contains, and guessing is what the annotation rule avoids.
        Expr::Input { span } => Err(Error::new(*span, "cannot tell what `input` contains")),

        // A mask over the subject Vec. The predicate is checked with `.` rebound to the element
        // type rather than evaluated in the enclosing scope.
        Expr::Select { pred, span } => {
            let Some(subject) = ctx.subject.clone() else {
                return Err(Error::new(*span, "`select` needs a subject, so it must follow `|`"));
            };
            let Some(elem) = subject.elem().cloned() else {
                return Err(Error::new(*span, format!("`select` needs a Vec, found {subject}")));
            };
            expect(&ctx.with(Some(elem)), pred, &Type::Bool)?;
            Ok(subject)
        }

        Expr::Call { func, func_span, arg, .. } => {
            let sig = ctx
                .sigs
                .get(func)
                .ok_or_else(|| Error::new(*func_span, format!("`{func}` is not a function")))?;
            expect(ctx, arg, &sig.param)?;
            Ok(sig.ret.clone())
        }

        Expr::Binary { op, lhs, rhs, span } => binary(ctx, *op, lhs, rhs, *span),
    }
}

fn binary(ctx: &Ctx, op: BinOp, lhs: &Expr, rhs: &Expr, span: crate::ast::Span) -> Result<Type, Error> {
    let left = synth(ctx, lhs)?;

    // Q2 is open, so an operator over a Vec is rejected rather than being silently given
    // broadcast or zip semantics. Under C1 that restriction is ordinary typing: there is no
    // separate cardinality to check, because a Vec is just a type.
    if left.elem().is_some() {
        return Err(Error::new(lhs.span(), format!("`{op}` does not apply to {left}")));
    }

    if op.is_comparison() {
        expect(ctx, rhs, &left)?;
        return Ok(Type::Bool);
    }

    // `+` is Str concatenation. Int has no arithmetic yet.
    expect(ctx, lhs, &Type::Str)?;
    expect(ctx, rhs, &Type::Str)?;
    let _ = span;
    Ok(Type::Str)
}

/// The checking direction. With no lambdas yet it synthesises and compares, but it is where the
/// expected type reaches the expression, and where the error names both sides.
fn expect(ctx: &Ctx, expr: &Expr, want: &Type) -> Result<(), Error> {
    // The one expression whose type comes from its position rather than its contents.
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
        return Ok(());
    }

    let found = synth(ctx, expr)?;
    if &found != want {
        return Err(Error::new(expr.span(), format!("expected {want}, found {found}")));
    }
    Ok(())
}
