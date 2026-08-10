use std::collections::HashMap;

use crate::ast::{BinOp, Expr, File, Span};
use crate::ir::{Func, Ir, Program};

/// The name the input value is bound to. Unspellable in source: `user` prefixes every source
/// name with `v_`.
pub const INPUT: &str = "t_input";

pub fn lower(file: &File, field_depths: &HashMap<Span, usize>) -> Program {
    let mut l = Lowerer { next: 0, depths: field_depths };
    let funcs = file
        .defs
        .iter()
        .map(|d| Func {
            name: user(&d.name),
            param: user(&d.param.name),
            body: l.expr(&d.body, None),
        })
        .collect();
    let body = l.expr(&file.body, None);
    Program { funcs, body }
}

/// A toylang name, made safe for the target. The target's namespace is not ours: a program with
/// a function called `print` or `end` would otherwise emit Lua that shadows the output function
/// or does not parse.
fn user(name: &str) -> String {
    format!("v_{name}")
}

struct Lowerer<'a> {
    next: usize,
    depths: &'a HashMap<Span, usize>,
}

impl Lowerer<'_> {
    /// A local the source cannot name. `user` prefixes every source name with `v_`, so nothing
    /// in a program can collide with `t_0`.
    fn fresh(&mut self) -> String {
        let n = self.next;
        self.next += 1;
        format!("t_{n}")
    }

    fn expr(&mut self, expr: &Expr, subject: Option<&str>) -> Ir {
        match expr {
            Expr::Str { text, .. } => Ir::ConstStr(text.clone()),
            Expr::Int { value, .. } => Ir::ConstInt(*value),
            Expr::Var { name, .. } => Ir::Local(user(name)),

            // The checker has already rejected a `.` with nothing to refer to.
            Expr::Subject { .. } => Ir::Local(subject.expect("checked").to_string()),

            Expr::VecLit { items, .. } => {
                Ir::VecLit(items.iter().map(|i| self.expr(i, subject)).collect())
            }

            // Projection by every index keeps the same extent, so there is nothing to emit.
            Expr::Project { base, .. } => self.expr(base, subject),

            Expr::Input { .. } => Ir::Local(INPUT.to_string()),

            Expr::Field { base, name, span } => Ir::Field {
                base: Box::new(self.expr(base, subject)),
                name: name.clone(),
                depth: *self.depths.get(span).expect("recorded by the checker"),
            },

            Expr::Pipe { lhs, rhs, .. } => {
                let value = self.expr(lhs, subject);
                let name = self.fresh();
                let body = self.expr(rhs, Some(&name));
                Ir::Bind { name, value: Box::new(value), body: Box::new(body) }
            }

            Expr::Select { pred, .. } => {
                let source = Ir::Local(subject.expect("checked").to_string());
                let param = self.fresh();
                let pred = self.expr(pred, Some(&param));
                Ir::Select { source: Box::new(source), param, pred: Box::new(pred) }
            }

            Expr::Call { func, arg, .. } => {
                Ir::Call { func: user(func), arg: Box::new(self.expr(arg, subject)) }
            }

            Expr::Binary { op: BinOp::Add, lhs, rhs, .. } => Ir::Concat(
                Box::new(self.expr(lhs, subject)),
                Box::new(self.expr(rhs, subject)),
            ),

            Expr::Binary { op, lhs, rhs, .. } => Ir::Compare {
                op: lua_op(*op),
                lhs: Box::new(self.expr(lhs, subject)),
                rhs: Box::new(self.expr(rhs, subject)),
            },
        }
    }
}

fn lua_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "==",
        BinOp::Ne => "~=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        BinOp::Add => unreachable!("Add lowers to Concat"),
    }
}
