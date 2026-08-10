use std::collections::HashMap;

use crate::ast::{BinOp, Def, Expr, File, TypeExpr};
use crate::error::Error;
use crate::ty::{Sig, Type};

/// Name to type. At most one entry today, since functions are unary and there is no `let`.
type Scope = [(String, Type)];

pub fn check(file: &File) -> Result<Type, Error> {
    let sigs = signatures(&file.defs)?;

    // Signatures are collected before any body is checked, so a definition may call one that
    // appears later in the file. This is also what recursion will need.
    for def in &file.defs {
        let sig = sigs[&def.name];
        let scope = [(def.param.name.clone(), sig.param)];
        let found = synth(&sigs, &scope, &def.body)?;
        if found != sig.ret {
            return Err(Error::new(
                def.body.span(),
                format!("`{}` declares it returns {}, but its body is {found}", def.name, sig.ret),
            ));
        }
    }

    synth(&sigs, &[], &file.body)
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
    Type::from_name(&ty.name)
        .ok_or_else(|| Error::new(ty.span, format!("unknown type `{}`", ty.name)))
}

fn synth(sigs: &HashMap<String, Sig>, scope: &Scope, expr: &Expr) -> Result<Type, Error> {
    match expr {
        Expr::Str { .. } => Ok(Type::Str),
        Expr::Int { .. } => Ok(Type::Int),

        Expr::Var { name, span } => scope
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, t)| *t)
            .ok_or_else(|| Error::new(*span, format!("`{name}` is not defined"))),

        Expr::Call { func, func_span, arg, .. } => {
            let sig = sigs
                .get(func)
                .ok_or_else(|| Error::new(*func_span, format!("`{func}` is not a function")))?;
            expect(sigs, scope, arg, sig.param)?;
            Ok(sig.ret)
        }

        // `+` is Str concatenation. Int exists as a type but has no arithmetic yet, so this is
        // the whole of what `+` accepts.
        Expr::Binary { op: BinOp::Add, lhs, rhs, .. } => {
            expect(sigs, scope, lhs, Type::Str)?;
            expect(sigs, scope, rhs, Type::Str)?;
            Ok(Type::Str)
        }
    }
}

/// The checking direction. With no lambdas yet it synthesises and compares, but it is where the
/// expected type reaches the expression, and where the error names both sides.
fn expect(
    sigs: &HashMap<String, Sig>,
    scope: &Scope,
    expr: &Expr,
    want: Type,
) -> Result<(), Error> {
    let found = synth(sigs, scope, expr)?;
    if found != want {
        return Err(Error::new(expr.span(), format!("expected {want}, found {found}")));
    }
    Ok(())
}
