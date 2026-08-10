use std::collections::HashMap;

use crate::ast::{BinOp, Def, Expr, File, TypeExpr};
use crate::error::Error;
use crate::ty::{Sig, Type};

struct Ctx<'a> {
    sigs: &'a HashMap<String, Sig>,
    /// Named bindings. At most one, since functions are unary and there is no `let`.
    scope: Vec<(String, Type)>,
    /// What `.` refers to here, if anything.
    subject: Option<Type>,
}

pub fn check(file: &File) -> Result<Type, Error> {
    let sigs = signatures(&file.defs)?;

    // Signatures are collected before any body is checked, so a definition may call one that
    // appears later in the file. This is also what recursion will need.
    for def in &file.defs {
        let sig = &sigs[&def.name];
        let ctx = Ctx {
            sigs: &sigs,
            scope: vec![(def.param.name.clone(), sig.param.clone())],
            subject: None,
        };
        let found = synth(&ctx, &def.body)?;
        if found != sig.ret {
            return Err(Error::new(
                def.body.span(),
                format!("`{}` declares it returns {}, but its body is {found}", def.name, sig.ret),
            ));
        }
    }

    let ctx = Ctx { sigs: &sigs, scope: Vec::new(), subject: None };
    synth(&ctx, &file.body)
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
            let inner = Ctx { sigs: ctx.sigs, scope: ctx.scope.clone(), subject: Some(subject) };
            synth(&inner, rhs)
        }

        // A mask over the subject Vec. The predicate is checked with `.` rebound to the element
        // type rather than evaluated in the enclosing scope.
        Expr::Select { pred, span } => {
            let Some(subject) = ctx.subject.clone() else {
                return Err(Error::new(*span, "`select` needs a subject, so it must follow `|`"));
            };
            let Some(elem) = subject.elem().cloned() else {
                return Err(Error::new(*span, format!("`select` needs a Vec, found {subject}")));
            };
            let inner =
                Ctx { sigs: ctx.sigs, scope: ctx.scope.clone(), subject: Some(elem) };
            expect(&inner, pred, &Type::Bool)?;
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
    let found = synth(ctx, expr)?;
    if &found != want {
        return Err(Error::new(expr.span(), format!("expected {want}, found {found}")));
    }
    Ok(())
}
