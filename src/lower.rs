use crate::ast::Expr;
use crate::ir::{Ir, Program};

pub fn lower(expr: &Expr) -> Program {
    let body = match expr {
        Expr::Str { text, .. } => Ir::ConstStr(text.clone()),
    };
    Program { body }
}
