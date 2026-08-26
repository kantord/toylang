use std::cell::{Cell, RefCell};
use std::collections::HashMap;

use crate::ast::{Alias, BinOp, Def, Expr, File, Span, TypeExpr};
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
    /// Whether `lines` has already been read. There is only ever one real stdin, so a second
    /// read is refused rather than silently handed nothing, the way a second pass over an
    /// already-consumed iterator would be in a language that let it compile.
    lines_used: &'a Cell<bool>,
    next_local: &'a Cell<LocalId>,
}

impl Ctx<'_> {
    fn with(&self, subject: Option<(Type, LocalId)>) -> Ctx<'_> {
        Ctx {
            sigs: self.sigs,
            scope: self.scope.clone(),
            subject,
            input: self.input,
            lines_used: self.lines_used,
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
    let lines_used = Cell::new(false);
    let aliases = alias_map(&file.aliases)?;
    // Resolved eagerly so a broken alias is an error even when nothing uses it, and so a cycle
    // is found here rather than wherever it happened to be reached from.
    for a in &file.aliases {
        resolve(&a.ty, &aliases, &mut vec![a.name.clone()])?;
    }
    let sigs = signatures(&file.defs, &aliases)?;
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
            lines_used: &lines_used,
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
        lines_used: &lines_used,
        next_local: &next_local,
    };
    let body = synth(&ctx, &file.body)?;
    // Lines cannot be printed, having nothing to show: it is a stream, not a value, and
    // collect() is what turns it into one. A function body catches this for free, since its
    // return annotation can never spell Lines and so can never match a body that contains one;
    // the program's own result has no annotation to check against, so it needs asking directly.
    if body.ty.contains_lines() {
        return Err(Error::new(
            file.body.span(),
            "the program's result contains `lines`, which has nothing to print; pass it to \
             `collect` first"
                .to_string(),
        ));
    }
    let input = input.into_inner();
    // Forced by jq specifically: reading `lines` needs raw-input mode for the whole invocation,
    // and raw-input mode is what stops `input` from being parsed as JSON at all. The other five
    // backends have no such conflict, but the language is one language across all six.
    if input.is_some() && lines_used.get() {
        return Err(Error::new(
            file.body.span(),
            "a program cannot use both `input` and `lines`; they read the same real stdin two \
             different ways"
                .to_string(),
        ));
    }
    Ok(tir::Program { funcs: prune_unreachable(funcs, &body), body, input, uses_lines: lines_used.get() })
}

/// Every function the program's body can actually reach, directly or through calls a reached
/// function itself makes. `pub fn`s from the prelude are always merged into `file.defs` before
/// this runs, so a `pub` one the program never calls needs pruning here to keep it out of a
/// backend's output and out of `tags::node_types` -- and an unused function the program wrote
/// itself is pruned by the same pass, for the same reason.
fn prune_unreachable(funcs: Vec<tir::Func>, body: &Tir) -> Vec<tir::Func> {
    let mut reached: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut worklist: Vec<String> = Vec::new();
    calls_in(body, &mut worklist);
    while let Some(name) = worklist.pop() {
        if reached.insert(name.clone())
            && let Some(f) = funcs.iter().find(|f| f.name == name)
        {
            calls_in(&f.body, &mut worklist);
        }
    }
    funcs.into_iter().filter(|f| reached.contains(&f.name)).collect()
}

/// Every function name a `Kind::Call` inside `t` names, collected recursively through every
/// other kind of node.
fn calls_in(t: &Tir, out: &mut Vec<String>) {
    if let Kind::Call { func, arg } = &t.kind {
        out.push(func.clone());
        calls_in(arg, out);
        return;
    }
    match &t.kind {
        Kind::Str(_)
        | Kind::Int(_)
        | Kind::Var(_)
        | Kind::Local(_)
        | Kind::Input
        | Kind::Lines => {}
        Kind::VecLit(items) => items.iter().for_each(|i| calls_in(i, out)),
        Kind::RecordLit { fields } => fields.iter().for_each(|(_, v)| calls_in(v, out)),
        Kind::Call { .. } => unreachable!("handled above"),
        Kind::Concat(l, r) => {
            calls_in(l, out);
            calls_in(r, out);
        }
        Kind::Arith { lhs, rhs, .. } | Kind::Compare { lhs, rhs, .. } => {
            calls_in(lhs, out);
            calls_in(rhs, out);
        }
        Kind::Cond { cond, then, otherwise } => {
            calls_in(cond, out);
            calls_in(then, out);
            calls_in(otherwise, out);
        }
        Kind::Bind { value, body, .. } | Kind::Map { source: value, body, .. } => {
            calls_in(value, out);
            calls_in(body, out);
        }
        Kind::Select { source, pred, .. } => {
            calls_in(source, out);
            calls_in(pred, out);
        }
        Kind::Field { base, .. } | Kind::Unwrap { base } => calls_in(base, out),
        Kind::Builtin { arg, .. } => calls_in(arg, out),
        Kind::Index { base, index, .. } => {
            calls_in(base, out);
            calls_in(index, out);
        }
    }
}

/// Functions the language provides. Unary like every other function, so they need no special
/// call syntax and are looked up before user definitions.
fn builtin(name: &str) -> Option<(tir::Builtin, Sig)> {
    let vec_of = |t: Type| Type::Vec(Box::new(t));
    Some(match name {
        "str" => (tir::Builtin::IntToStr, Sig { param: Type::Int, ret: Type::Str }),
        "range" => (tir::Builtin::Range, Sig { param: Type::Int, ret: vec_of(Type::Int) }),
        "collect" => (tir::Builtin::Collect, Sig { param: Type::Lines, ret: vec_of(Type::Str) }),
        _ => return None,
    })
}

/// Capitalised names are types and lowercase names are values.
///
/// Every name in the language already followed this, unenforced. Making it a rule is what lets a
/// constructor share a spelling with a call without sharing a namespace: `User {..}` builds and
/// `area {..}` calls, decided by the first letter rather than by looking either one up.
///
/// Field names are deliberately not covered. They come from data, and JSON objects are entitled
/// to a key spelled `Name`.
fn value_name(name: &str, span: Span, what: &str) -> Result<(), Error> {
    if name.chars().next().is_some_and(char::is_uppercase) {
        return Err(Error::new(
            span,
            format!("a {what} starts with a lowercase letter, and `{name}` reads as a type"),
        ));
    }
    Ok(())
}

/// What a type name stands for. An alias is an abbreviation, so this maps to the written form
/// and `resolve` expands it; nothing downstream ever learns a name was involved.
type Aliases<'a> = HashMap<String, &'a TypeExpr>;

fn alias_map(aliases: &[Alias]) -> Result<Aliases<'_>, Error> {
    let mut map: Aliases = HashMap::new();
    for a in aliases {
        if !a.name.chars().next().is_some_and(char::is_uppercase) {
            return Err(Error::new(
                a.span,
                format!("a type name starts with a capital letter, and `{}` reads as a value", a.name),
            ));
        }
        if Type::from_name(&a.name).is_some() || a.name == "Vec" || a.name == "Opt" {
            return Err(Error::new(
                a.span,
                format!("`{}` is a built-in type and cannot be redefined", a.name),
            ));
        }
        if map.insert(a.name.clone(), &a.ty).is_some() {
            return Err(Error::new(a.span, format!("type `{}` is defined twice", a.name)));
        }
    }
    Ok(map)
}

fn signatures(defs: &[Def], aliases: &Aliases) -> Result<HashMap<String, Sig>, Error> {
    let mut sigs = HashMap::new();
    for def in defs {
        value_name(&def.name, def.span, "function name")?;
        value_name(&def.param.name, def.param.span, "parameter name")?;
        // `jsonlines`, `extent`, `concat`, `tail`, `select`, and `map` are not in `builtin()`'s
        // fixed table -- the first four are polymorphic, the last two rebind `.` -- but all six
        // are reserved names for the same reason every other builtin is.
        if builtin(&def.name).is_some()
            || matches!(
                def.name.as_str(),
                "jsonlines" | "extent" | "concat" | "tail" | "select" | "map"
            )
        {
            return Err(Error::new(
                def.span,
                format!("`{}` is a builtin and cannot be redefined", def.name),
            ));
        }
        if sigs.contains_key(&def.name) {
            return Err(Error::new(def.span, format!("`{}` is defined twice", def.name)));
        }
        let sig = Sig {
            param: resolve(&def.param.ty, aliases, &mut Vec::new())?,
            ret: resolve(&def.ret, aliases, &mut Vec::new())?,
        };
        sigs.insert(def.name.clone(), sig);
    }
    Ok(sigs)
}

/// `seen` is the chain of alias names currently being expanded, so a type written in terms of
/// itself is caught rather than expanded forever.
fn resolve(ty: &TypeExpr, aliases: &Aliases, seen: &mut Vec<String>) -> Result<Type, Error> {
    match ty {
        TypeExpr::Named { name, span } => {
            if let Some(built_in) = Type::from_name(name) {
                return Ok(built_in);
            }
            let Some(written) = aliases.get(name) else {
                return Err(Error::new(*span, format!("unknown type `{name}`")));
            };
            if let Some(at) = seen.iter().position(|s| s == name) {
                // The names expanded since this one last appeared are the cycle, and naming them
                // is the difference between knowing there is one and finding it.
                let through: Vec<String> =
                    seen[at + 1..].iter().map(|s| format!("`{s}`")).collect();
                let path = if through.is_empty() {
                    String::new()
                } else {
                    format!(", through {}", through.join(" and "))
                };
                return Err(Error::new(
                    *span,
                    format!("type `{name}` is written in terms of itself{path}"),
                ));
            }
            seen.push(name.clone());
            let expanded = resolve(written, aliases, seen)?;
            seen.pop();
            Ok(expanded)
        }
        TypeExpr::Vec { elem, .. } => Ok(Type::Vec(Box::new(resolve(elem, aliases, seen)?))),
        TypeExpr::Record { fields, span } => {
            let mut out = Vec::new();
            for (name, ty) in fields {
                if out.iter().any(|(n, _): &(String, Type)| n == name) {
                    return Err(Error::new(*span, format!("field `{name}` is declared twice")));
                }
                out.push((name.clone(), resolve(ty, aliases, seen)?));
            }
            Ok(Type::record(out))
        }
    }
}

fn synth(ctx: &Ctx, expr: &Expr) -> Result<Tir, Error> {
    match expr {
        Expr::Str { text, .. } => Ok(Tir::new(Type::Str, Kind::Str(text.clone()))),
        Expr::Lines { span } => {
            if ctx.lines_used.get() {
                return Err(Error::new(
                    *span,
                    "`lines` has already been read; there is only one stdin".to_string(),
                ));
            }
            ctx.lines_used.set(true);
            Ok(Tir::new(Type::Lines, Kind::Lines))
        }
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

        Expr::RecordLit { fields, .. } => {
            let mut built: Vec<(String, Tir)> = Vec::new();
            for (name, name_span, value) in fields {
                if built.iter().any(|(seen, _)| seen == name) {
                    return Err(Error::new(
                        *name_span,
                        format!("field `{name}` is given twice"),
                    ));
                }
                let field = synth(ctx, value)?;
                // Lines can never enter a record: a record's fields can be printed, copied,
                // and read back out by name, none of which make sense for a single-use stream,
                // and no path exists for a `Lines`-typed field to leave one again once inside.
                if field.ty.contains_lines() {
                    return Err(Error::new(
                        value.span(),
                        format!("`{name}` cannot be `lines`, which has nothing to store"),
                    ));
                }
                built.push((name.clone(), field));
            }
            // Sorted to match Type::record, so a field's index is the same in the value and
            // in the type.
            built.sort_by(|a, b| a.0.cmp(&b.0));
            let ty =
                Type::record(built.iter().map(|(n, t)| (n.clone(), t.ty.clone())).collect());
            Ok(Tir::new(ty, Kind::RecordLit { fields: built }))
        }

        Expr::VecLit { items, span } => {
            let Some(first) = items.first() else {
                // Nothing says what an empty literal contains, and there is no expected type to
                // supply it. Guessing here is what the annotation rule exists to avoid.
                return Err(Error::new(*span, "cannot tell what `[]` contains"));
            };
            let head = synth(ctx, first)?;
            let elem = head.ty.clone();
            // Same reasoning as a record field: nothing can get a `Lines` value back out of a
            // Vec once it is in one, and a Vec of them makes no sense when there is only ever
            // one real stdin to begin with.
            if elem.contains_lines() {
                return Err(Error::new(
                    first.span(),
                    "a Vec cannot hold `lines`, which has nothing to store".to_string(),
                ));
            }
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

        Expr::Call { func, func_span, arg, span } => {
            // `select` and `map` are not special syntax, only special names: they are ordinary
            // calls whose argument is checked with `.` rebound to the subject's element type
            // instead of evaluated in the enclosing scope, which no ordinary function needs and
            // is why they cannot be defined as one (see `signatures`).
            if func == "select" {
                let Some((subject, id)) = ctx.subject.clone() else {
                    return Err(Error::new(*span, "`select` needs a subject, so it must follow `|`"));
                };
                let Some(elem) = subject.elem().cloned() else {
                    return Err(Error::new(*span, format!("`select` needs a Vec, found {subject}")));
                };
                let param = ctx.fresh();
                let pred = expect(&ctx.with(Some((elem, param))), arg, &Type::Bool)?;
                let source = Tir::new(subject.clone(), Kind::Local(id));
                return Ok(Tir::new(
                    subject,
                    Kind::Select { source: Box::new(source), param, pred: Box::new(pred) },
                ));
            }
            // The one way to produce a new element value. `select` removes elements and a field
            // access reads a field; neither can turn a Vec<Int> into a Vec<Str>.
            if func == "map" {
                let Some((subject, id)) = ctx.subject.clone() else {
                    return Err(Error::new(*span, "`map` needs a subject, so it must follow `|`"));
                };
                let Some(elem) = subject.elem().cloned() else {
                    return Err(Error::new(*span, format!("`map` needs a Vec, found {subject}")));
                };
                let param = ctx.fresh();
                let body = synth(&ctx.with(Some((elem, param))), arg)?;
                let source = Tir::new(subject, Kind::Local(id));
                return Ok(Tir::new(
                    Type::Vec(Box::new(body.ty.clone())),
                    Kind::Map { source: Box::new(source), param, body: Box::new(body) },
                ));
            }
            // Polymorphic over the element type, so there is no one fixed Sig to check the
            // argument against the way every other builtin has: the argument is synthesised
            // first, and only then is its shape checked, the reverse of the usual direction.
            if func == "jsonlines" {
                let arg_span = arg.span();
                let arg = synth(ctx, arg)?;
                if arg.ty.elem().is_none() {
                    return Err(Error::new(
                        arg_span,
                        format!("`jsonlines` needs a Vec, found {}", arg.ty),
                    ));
                }
                return Ok(Tir::new(
                    Type::Str,
                    Kind::Builtin { which: tir::Builtin::JsonLines, arg: Box::new(arg) },
                ));
            }
            // `extent`, `tail`, and `concat` are polymorphic over the element type, the same
            // reason `jsonlines` is checked here rather than through `builtin()`'s fixed table.
            if func == "extent" {
                let arg_span = arg.span();
                let arg = synth(ctx, arg)?;
                if arg.ty.elem().is_none() {
                    return Err(Error::new(
                        arg_span,
                        format!("`extent` needs a Vec, found {}", arg.ty),
                    ));
                }
                return Ok(Tir::new(
                    Type::Int,
                    Kind::Builtin { which: tir::Builtin::Extent, arg: Box::new(arg) },
                ));
            }
            // `None` on an empty Vec, the same way `Index` turns reaching past what's there
            // into `Opt` rather than a runtime failure.
            if func == "tail" {
                let arg_span = arg.span();
                let arg = synth(ctx, arg)?;
                let Some(elem) = arg.ty.elem().cloned() else {
                    return Err(Error::new(
                        arg_span,
                        format!("`tail` needs a Vec, found {}", arg.ty),
                    ));
                };
                return Ok(Tir::new(
                    Type::Opt(Box::new(Type::Vec(Box::new(elem)))),
                    Kind::Builtin { which: tir::Builtin::Tail, arg: Box::new(arg) },
                ));
            }
            // Flattens a Vec<Vec<T>> into a Vec<T>, the way jq's `add` flattens a list of
            // arrays. A named function rather than `+` on Vec, so it does not decide Q2.
            if func == "concat" {
                let arg_span = arg.span();
                let arg = synth(ctx, arg)?;
                let inner = arg.ty.elem().cloned();
                let Some(elem) = inner.as_ref().and_then(Type::elem).cloned() else {
                    return Err(Error::new(
                        arg_span,
                        format!("`concat` needs a Vec of Vecs, found {}", arg.ty),
                    ));
                };
                return Ok(Tir::new(
                    Type::Vec(Box::new(elem)),
                    Kind::Builtin { which: tir::Builtin::Concat, arg: Box::new(arg) },
                ));
            }
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
/// and adds one to the depth; a field access reads a field of it and leaves the depth alone.
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
