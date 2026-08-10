use crate::ast::Expr;
use crate::error::Error;
use crate::ty::Type;

/// Synthesis only. The checking direction arrives at step 3, with the first expected type.
pub fn check(expr: &Expr) -> Result<Type, Error> {
    match expr {
        Expr::Str { .. } => Ok(Type::Str),
        // `+` is Str concatenation. Operands are walked for their own errors, but their types
        // cannot disagree while Str is the only type; step 4 adds Int and the real check.
        Expr::Binary { lhs, rhs, .. } => {
            check(lhs)?;
            check(rhs)?;
            Ok(Type::Str)
        }
    }
}
