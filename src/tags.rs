//! Human-readable tags for the AST shapes a program exercises.
//!
//! Drawn from `tir::Kind`, not `ast::Expr`: `select`, `map`, and even `+` only resolve into
//! their real meaning once type checking has run (`ast::Expr::Call` covers a user function, a
//! builtin, and `select`/`map` alike, and `+` covers both `Concat` and `Arith::Add`). Every
//! corpus case compiles, so `Tir` is always available.
//!
//! Where CONTEXT.md already names a concept, the tag uses that name -- `application` for a call,
//! `projection` for a field access, `selection` for both `select(...)` and an index, since the
//! glossary treats those as the same idea under different specs. `map-over` is the one deliberate
//! departure: CONTEXT.md reserves `Map` for the not-yet-built dict-like type, so the `map(f)`
//! builtin gets a different word rather than colliding with it.

use std::collections::BTreeSet;

use crate::ast::BinOp;
use crate::tir::{Builtin, Kind, Program, Tir};

/// Every tag exercised by `program`, sorted and deduplicated. A tag with no sub-case is its bare
/// name (`projection`); one with a sub-case is `parent.child` (`arith.add`, `builtin.range`).
pub fn node_types(program: &Program) -> Vec<String> {
    let mut tags = BTreeSet::new();
    for func in &program.funcs {
        walk(&func.body, &mut tags);
    }
    walk(&program.body, &mut tags);
    tags.into_iter().collect()
}

fn walk(tir: &Tir, tags: &mut BTreeSet<String>) {
    tags.insert(tag(tir));
    match &tir.kind {
        Kind::Str(_)
        | Kind::Int(_)
        | Kind::Var(_)
        | Kind::Local(_)
        | Kind::Input
        | Kind::Inputs
        | Kind::Lines => {}
        Kind::VecLit(items) => items.iter().for_each(|i| walk(i, tags)),
        Kind::RecordLit { fields } => fields.iter().for_each(|(_, v)| walk(v, tags)),
        Kind::EnumLit { payload, .. } => {
            if let Some(p) = payload {
                walk(p, tags);
            }
        }
        Kind::Call { arg, .. } => {
            if let Some(a) = arg {
                walk(a, tags);
            }
        }
        Kind::Concat(lhs, rhs) => {
            walk(lhs, tags);
            walk(rhs, tags);
        }
        Kind::Arith { lhs, rhs, .. }
        | Kind::Compare { lhs, rhs, .. }
        | Kind::Logic { lhs, rhs, .. } => {
            walk(lhs, tags);
            walk(rhs, tags);
        }
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => {
            walk(cond, tags);
            walk(then, tags);
            walk(otherwise, tags);
        }
        Kind::Bind { value, body, .. }
        | Kind::Map {
            source: value,
            body,
            ..
        }
        | Kind::OptMap {
            source: value,
            body,
            ..
        } => {
            walk(value, tags);
            walk(body, tags);
        }
        Kind::Select { source, pred, .. } => {
            walk(source, tags);
            walk(pred, tags);
        }
        Kind::Field { base, .. } | Kind::Unwrap { base } | Kind::Not(base) => walk(base, tags),
        Kind::Builtin { arg, .. } => walk(arg, tags),
        Kind::Index { base, index, .. } => {
            walk(base, tags);
            walk(index, tags);
        }
        Kind::Match { subject, arms, .. } => {
            walk(subject, tags);
            for a in arms {
                if let Some(g) = &a.guard {
                    walk(g, tags);
                }
                walk(&a.body, tags);
            }
        }
    }
}

fn tag(tir: &Tir) -> String {
    match &tir.kind {
        Kind::Str(_) => "str".into(),
        // One literal kind, two widths: which one this literal has is the node's type's to
        // say (kantord/toylang#83), so the tag reads it rather than the kind.
        Kind::Int(_) if tir.ty == crate::ty::Type::Int64 => "int64".into(),
        Kind::Int(_) => "int".into(),
        Kind::VecLit(_) => "vec-literal".into(),
        Kind::RecordLit { .. } => "record-literal".into(),
        // CONTEXT.md's terms: a `variant` is one alternative, a `unit variant` carries nothing.
        Kind::EnumLit { payload: None, .. } => "variant.unit".into(),
        Kind::EnumLit {
            payload: Some(_), ..
        } => "variant.payload".into(),
        Kind::Var(_) => "var".into(),
        Kind::Local(_) => "local".into(),
        Kind::Input => "input".into(),
        Kind::Lines => "lines".into(),
        Kind::Call { .. } => "application".into(),
        Kind::Concat(..) => "concat".into(),
        Kind::Arith { op, .. } => format!("arith.{}", binop_tag(*op)),
        Kind::Cond { .. } => "conditional".into(),
        Kind::Compare { op, .. } => format!("compare.{}", binop_tag(*op)),
        Kind::Logic { op, .. } => format!("logic.{op}"),
        Kind::Not(_) => "logic.not".into(),
        Kind::Bind { .. } => "pipe".into(),
        Kind::Map { .. } => "map-over".into(),
        Kind::OptMap { .. } => "opt-map".into(),
        Kind::Select { .. } => "selection.narrow".into(),
        Kind::Field { .. } => "projection".into(),
        Kind::Builtin { which, .. } => format!("builtin.{}", builtin_tag(*which)),
        Kind::Unwrap { .. } => "unwrap".into(),
        Kind::Index { .. } => "selection.collapse".into(),
        Kind::Inputs => "inputs".into(),
        Kind::Match { .. } => "match".into(),
    }
}

fn binop_tag(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "add",
        BinOp::Sub => "sub",
        BinOp::Mul => "mul",
        BinOp::Div => "div",
        BinOp::Rem => "rem",
        BinOp::Eq => "eq",
        BinOp::Ne => "ne",
        BinOp::Lt => "lt",
        BinOp::Le => "le",
        BinOp::Gt => "gt",
        BinOp::Ge => "ge",
    }
}

fn builtin_tag(which: Builtin) -> &'static str {
    match which {
        Builtin::IntToStr => "str",
        Builtin::IntToI64 => "i64",
        Builtin::Range => "range",
        Builtin::Collect => "collect",
        Builtin::JsonLines => "jsonlines",
        Builtin::Length => "length",
        Builtin::Flatten => "flatten",
        Builtin::Tail => "tail",
        Builtin::Fields => "fields",
        Builtin::Chars => "chars",
        Builtin::Sort => "sort",
        Builtin::Reverse => "reverse",
    }
}
