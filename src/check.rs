use crate::ast::Expr;
use crate::error::Error;
use crate::ty::Type;

/// Synthesis only. The checking direction arrives at step 3, with the first expected type.
pub fn check(expr: &Expr) -> Result<Type, Error> {
    match expr {
        Expr::Str { .. } => Ok(Type::Str),
    }
}
