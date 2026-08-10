use crate::ast::{BinOp, Expr, File};
use crate::ir::{Func, Ir, Program};

pub fn lower(file: &File) -> Program {
    let funcs = file
        .defs
        .iter()
        .map(|d| Func {
            name: d.name.clone(),
            param: d.param.name.clone(),
            body: lower_expr(&d.body),
        })
        .collect();
    Program { funcs, body: lower_expr(&file.body) }
}

fn lower_expr(expr: &Expr) -> Ir {
    match expr {
        Expr::Str { text, .. } => Ir::ConstStr(text.clone()),
        Expr::Int { value, .. } => Ir::ConstInt(*value),
        Expr::Var { name, .. } => Ir::Var(name.clone()),
        Expr::Call { func, arg, .. } => {
            Ir::Call { func: func.clone(), arg: Box::new(lower_expr(arg)) }
        }
        Expr::Binary { op: BinOp::Add, lhs, rhs, .. } => {
            Ir::Concat(Box::new(lower_expr(lhs)), Box::new(lower_expr(rhs)))
        }
    }
}
