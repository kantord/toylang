use crate::ast::{BinOp, Expr};
use crate::ir::{Ir, Program};

pub fn lower(expr: &Expr) -> Program {
    Program { body: lower_expr(expr) }
}

fn lower_expr(expr: &Expr) -> Ir {
    match expr {
        Expr::Str { text, .. } => Ir::ConstStr(text.clone()),
        Expr::Binary { op: BinOp::Add, lhs, rhs, .. } => {
            Ir::Concat(Box::new(lower_expr(lhs)), Box::new(lower_expr(rhs)))
        }
    }
}
