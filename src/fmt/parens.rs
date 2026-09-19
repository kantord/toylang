//! Minimal-but-correct parenthesization. The AST does not record which parens the source used,
//! so every paren in the output is one this module decided it needed, by comparing a node's own
//! operator power against the power the position it sits in requires -- the same table
//! `parse.rs::infix_power` parses with, walked in reverse. `Ctx` is the position; `needs_parens`
//! is the decision.

use crate::ast::{BinOp, Expr, LogicOp};

// Mirrors parse.rs's precedence table exactly (`infix_power`, `PIPE_LEFT`/`PIPE_RIGHT`,
// `COND_POWER`, `OR_LEFT`/`OR_RIGHT`, `NOT_POWER`): the numbers a paren decision here has to
// answer to are the parser's, not this module's own invention.
const PIPE_LEFT: u8 = 1;

pub(super) const PIPE_RIGHT: u8 = 2;

pub(super) const COND_POWER: u8 = 3;

pub(super) const NOT_POWER: u8 = 8;

/// A match arm's body is printed at a power just above `or`'s, which is how the parser's second
/// reading of `or` -- the arm separator, which has no power at all -- reaches this table: a bare
/// disjunction in an arm body is the one expression whose parens the powers alone would not ask
/// for, and the one the parser would otherwise read as two arms.
pub(super) const ARM_BODY: u8 = 5;

pub(super) fn bin_power(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => (8, 9),
        BinOp::Add | BinOp::Sub => (10, 11),
        BinOp::Mul | BinOp::Div | BinOp::Rem => (12, 13),
    }
}

pub(super) fn logic_power(op: LogicOp) -> (u8, u8) {
    match op {
        LogicOp::Or => (4, 5),
        LogicOp::And => (6, 7),
    }
}

/// Where a child expression sits, in terms of what the parser would have accepted bare at that
/// spot -- everything `needs_parens` and the wrapping printer need to know to reproduce the tree
/// exactly, and nothing else.
#[derive(Clone, Copy)]
pub(super) enum Ctx {
    /// A fresh `self.expr(m)` call: File/Def body, a call argument (always real parens here, so
    /// always reset to `Expr(0)` inside them), `Index`'s bracketed expression, a match arm's
    /// body. The one context where a bare `Pipe` and, when `m` is loose enough, a bare `Match`
    /// are both reachable.
    Expr(u8),
    /// A `self.operand(m)` call: a `Binary` child, or `Pattern::Guard`'s expression. `Pipe` and
    /// `Match` are never reachable bare here -- only `expr()` parses those.
    Operand(u8),
    /// The base of a postfix chain (`Field`/`Index`/`Project`/`Unwrap`), or a `Call`/`Variant`'s
    /// argument print used only when NOT already inside real parens. Nothing compound is
    /// reachable bare here -- not even `Neg`.
    Atom,
    /// `Neg`'s base: like `Atom`, except a nested `Neg` is reachable bare (`- -a`).
    Unary,
}

/// The left binding power of whatever operator `e` is, for the three contexts that decide by
/// power alone. `None` for everything else, which each context rules on by kind.
fn left_power(e: &Expr) -> Option<u8> {
    match e {
        Expr::Binary { op, .. } => Some(bin_power(*op).0),
        Expr::Logic { op, .. } => Some(logic_power(*op).0),
        Expr::Not { .. } => Some(NOT_POWER),
        _ => None,
    }
}

pub(super) fn needs_parens(e: &Expr, ctx: Ctx) -> bool {
    // A `let` block is only ever a function body, never a sub-expression, so no child position
    // can hold one; treating it as needing parens everywhere is the safe dead arm.
    if matches!(e, Expr::Let { .. }) {
        return true;
    }
    match ctx {
        Ctx::Atom => matches!(
            e,
            Expr::Binary { .. }
                | Expr::Logic { .. }
                | Expr::Not { .. }
                | Expr::Pipe { .. }
                | Expr::Match { .. }
                | Expr::Neg { .. }
        ),
        Ctx::Unary => {
            matches!(
                e,
                Expr::Binary { .. }
                    | Expr::Logic { .. }
                    | Expr::Not { .. }
                    | Expr::Pipe { .. }
                    | Expr::Match { .. }
            )
        }
        Ctx::Operand(m) => match e {
            Expr::Pipe { .. } | Expr::Match { .. } => true,
            _ => left_power(e).is_some_and(|p| p < m),
        },
        Ctx::Expr(m) => match e {
            Expr::Pipe { .. } => PIPE_LEFT < m,
            Expr::Match { .. } => m > PIPE_RIGHT,
            _ => left_power(e).is_some_and(|p| p < m),
        },
    }
}
