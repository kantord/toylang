//! Two passes that walk an already-built `Tir` and never touch `Expr` or type resolution: stream
//! linearity (every stream-typed binding consumed exactly once) and dead-code pruning (which
//! functions the program body can actually reach).

use crate::ast::Span;
use crate::error::Error;
use crate::tir::{self, Kind, LocalId, Tir};

/// What a linearity count is looking for: a stream-typed binding, named either by the source
/// (a function parameter) or by the checker (the local a `|` binds `.` to).
pub(super) enum StreamBinding<'a> {
    Param(&'a str),
    Local(LocalId),
}

/// What broke the exactly-once rule, when a plain count cannot say it.
enum LinearViolation {
    /// A conditional's or match's paths disagree on how often they consume the binding.
    Branches { first: usize, second: usize },
    /// The binding is consumed inside a mapper's body, which runs once per element -- one
    /// spelled consumption, many runtime ones.
    InMapper,
}

/// How many times `t` consumes `binding`, counted along one evaluation path: a conditional
/// runs one branch, so its branches must agree with each other rather than being summed, and
/// a match's arms likewise. A mapper's body runs once per element, so any consumption there
/// is its own violation rather than a count.
fn stream_uses(t: &Tir, binding: &StreamBinding) -> Result<usize, LinearViolation> {
    let both = |a: &Tir, b: &Tir| Ok(stream_uses(a, binding)? + stream_uses(b, binding)?);
    match &t.kind {
        Kind::Var(name) => Ok(match binding {
            StreamBinding::Param(p) => (p == name) as usize,
            StreamBinding::Local(_) => 0,
        }),
        Kind::Local(id) => Ok(match binding {
            StreamBinding::Local(l) => (l == id) as usize,
            StreamBinding::Param(_) => 0,
        }),
        Kind::Str(_)
        | Kind::Int(_)
        | Kind::Float(_)
        | Kind::Input
        | Kind::Inputs
        | Kind::Lines
        | Kind::Dsv { .. } => Ok(0),
        Kind::VecLit(items) => items
            .iter()
            .try_fold(0, |n, i| Ok(n + stream_uses(i, binding)?)),
        Kind::RecordLit { fields } => fields
            .iter()
            .try_fold(0, |n, (_, v)| Ok(n + stream_uses(v, binding)?)),
        Kind::EnumLit { payload, .. } => payload
            .as_deref()
            .map_or(Ok(0), |p| stream_uses(p, binding)),
        Kind::Call { arg, .. } => arg.as_deref().map_or(Ok(0), |a| stream_uses(a, binding)),
        Kind::Builtin { arg, .. } => stream_uses(arg, binding),
        Kind::Concat(l, r) => both(l, r),
        Kind::Arith { lhs, rhs, .. } | Kind::Compare { lhs, rhs, .. } => both(lhs, rhs),
        // Summed rather than reconciled the way a conditional's branches are: `and`/`or` may
        // skip their right side, so a binding consumed on both sides is consumed once or twice
        // depending on the left operand's value, and "sometimes twice" is exactly what the
        // exactly-once rule refuses.
        Kind::Logic { lhs, rhs, .. } => both(lhs, rhs),
        Kind::Not(base) => stream_uses(base, binding),
        Kind::Bind { value, body, .. } => both(value, body),
        Kind::Map { source, body, .. } | Kind::OptMap { source, body, .. } => {
            if stream_uses(body, binding)? > 0 {
                return Err(LinearViolation::InMapper);
            }
            stream_uses(source, binding)
        }
        Kind::Select { source, pred, .. } => {
            if stream_uses(pred, binding)? > 0 {
                return Err(LinearViolation::InMapper);
            }
            stream_uses(source, binding)
        }
        Kind::Field { base, .. } | Kind::Unwrap { base } => stream_uses(base, binding),
        Kind::Index { base, index, .. } => both(base, index),
        Kind::Slice {
            base, start, end, ..
        } => {
            let mut acc = stream_uses(base, binding)?;
            if let Some(s) = start {
                acc += stream_uses(s, binding)?;
            }
            if let Some(e) = end {
                acc += stream_uses(e, binding)?;
            }
            Ok(acc)
        }
        Kind::Match {
            subject,
            arms,
            partial,
        } => {
            // A path through the chain evaluates every guard up to its arm, then that arm's
            // body; a partial chain has one more path, the fall-through, which evaluates every
            // guard and no body. All of them must agree.
            let mut guards_so_far = 0;
            let mut counts: Vec<usize> = Vec::new();
            for a in arms {
                if let Some(g) = &a.guard {
                    guards_so_far += stream_uses(g, binding)?;
                }
                counts.push(guards_so_far + stream_uses(&a.body, binding)?);
            }
            if *partial {
                counts.push(guards_so_far);
            }
            if let Some(w) = counts.windows(2).find(|w| w[0] != w[1]) {
                return Err(LinearViolation::Branches {
                    first: w[0],
                    second: w[1],
                });
            }
            Ok(stream_uses(subject, binding)? + counts.first().copied().unwrap_or(0))
        }
    }
}

/// The per-binding half of stream linearity: `binding`, already known to be stream-typed, must
/// be consumed exactly once by `body`. Zero uses is an error too -- linear, not affine: a
/// dropped stream is the Python silent-empty-generator mistake, and exactly-once can relax to
/// at-most-once later without breaking a program, while the reverse tightening could not.
pub(super) fn check_linear(
    body: &Tir,
    binding: &StreamBinding,
    subject: &str,
    span: Span,
) -> Result<(), Error> {
    match stream_uses(body, binding) {
        Err(LinearViolation::Branches { first, second }) => Err(Error::new(
            span,
            format!(
                "{subject} must be consumed exactly once on every path, but one branch \
                 consumes it {first} times and another {second}"
            ),
        )),
        Err(LinearViolation::InMapper) => Err(Error::new(
            span,
            format!(
                "{subject} must be consumed exactly once, but here it is consumed inside a \
                 mapper body, which runs once per element"
            ),
        )),
        Ok(1) => Ok(()),
        Ok(0) => Err(Error::new(
            span,
            format!("{subject} must be consumed exactly once; it is never consumed"),
        )),
        Ok(n) => Err(Error::new(
            span,
            format!("{subject} must be consumed exactly once, not {n} times"),
        )),
    }
}

/// Whether any node in `t` satisfies `pred`, walking every child a Tir node can hold. The same
/// shape as `stream_uses`, but a plain OR rather than a count: unused-binding checks only need
/// to know whether a name was read at all, not how many times or along which path.
fn any_node(t: &Tir, pred: &dyn Fn(&Tir) -> bool) -> bool {
    if pred(t) {
        return true;
    }
    match &t.kind {
        Kind::Var(_)
        | Kind::Local(_)
        | Kind::Str(_)
        | Kind::Int(_)
        | Kind::Float(_)
        | Kind::Input
        | Kind::Inputs
        | Kind::Lines
        | Kind::Dsv { .. } => false,
        Kind::VecLit(items) => items.iter().any(|i| any_node(i, pred)),
        Kind::RecordLit { fields } => fields.iter().any(|(_, v)| any_node(v, pred)),
        Kind::EnumLit { payload, .. } => payload.as_deref().is_some_and(|p| any_node(p, pred)),
        Kind::Call { arg, .. } => arg.as_deref().is_some_and(|a| any_node(a, pred)),
        Kind::Builtin { arg, .. } => any_node(arg, pred),
        Kind::Concat(l, r) => any_node(l, pred) || any_node(r, pred),
        Kind::Not(base) => any_node(base, pred),
        Kind::Arith { lhs, rhs, .. }
        | Kind::Compare { lhs, rhs, .. }
        | Kind::Logic { lhs, rhs, .. } => any_node(lhs, pred) || any_node(rhs, pred),
        Kind::Bind { value, body, .. } => any_node(value, pred) || any_node(body, pred),
        Kind::Map { source, body, .. } | Kind::OptMap { source, body, .. } => {
            any_node(source, pred) || any_node(body, pred)
        }
        Kind::Select {
            source, pred: p, ..
        } => any_node(source, pred) || any_node(p, pred),
        Kind::Field { base, .. } | Kind::Unwrap { base } => any_node(base, pred),
        Kind::Index { base, index, .. } => any_node(base, pred) || any_node(index, pred),
        Kind::Slice {
            base, start, end, ..
        } => {
            any_node(base, pred)
                || start.as_deref().is_some_and(|s| any_node(s, pred))
                || end.as_deref().is_some_and(|e| any_node(e, pred))
        }
        Kind::Match { subject, arms, .. } => {
            any_node(subject, pred)
                || arms.iter().any(|a| {
                    a.guard.as_ref().is_some_and(|g| any_node(g, pred)) || any_node(&a.body, pred)
                })
        }
    }
}

/// Whether the function parameter `name` is read anywhere in its body. Go exempts parameters
/// from its unused-binding rule; toylang does not, since a function here has exactly one, and
/// declaring an input the body ignores is the same dead-code smell the rule exists for.
pub(super) fn param_used(body: &Tir, name: &str) -> bool {
    any_node(body, &|t| matches!(&t.kind, Kind::Var(n) if n == name))
}

/// Whether a match arm's destructured field `name`, bound off payload local `pid`, is read
/// anywhere in the arm's body.
pub(super) fn field_used(body: &Tir, pid: LocalId, name: &str) -> bool {
    any_node(
        body,
        &|t| matches!(&t.kind, Kind::Field { base, name: n } if n == name && matches!(base.kind, Kind::Local(id) if id == pid)),
    )
}

/// Every function the program's body can actually reach, directly or through calls a reached
/// function itself makes. `pub fn`s from the prelude are always merged into `file.defs` before
/// this runs, so a `pub` one the program never calls needs pruning here to keep it out of a
/// backend's output and out of `tags::node_types` -- and an unused function the program wrote
/// itself is pruned by the same pass, for the same reason.
pub(super) fn prune_unreachable(funcs: Vec<tir::Func>, body: &Tir) -> Vec<tir::Func> {
    let mut reached: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut worklist: Vec<String> = Vec::new();
    calls_in(body, &mut worklist);
    while let Some(name) = worklist.pop() {
        if reached.insert(name.clone())
            && let Some(f) = funcs.iter().find(|f| f.name == name)
        {
            calls_in(&f.body, &mut worklist);
        }
    }
    funcs
        .into_iter()
        .filter(|f| reached.contains(&f.name))
        .collect()
}

/// Every function name a `Kind::Call` inside `t` names, collected recursively through every
/// other kind of node.
fn calls_in(t: &Tir, out: &mut Vec<String>) {
    if let Kind::Call { func, arg } = &t.kind {
        out.push(func.clone());
        if let Some(arg) = arg {
            calls_in(arg, out);
        }
        return;
    }
    match &t.kind {
        Kind::Str(_)
        | Kind::Int(_)
        | Kind::Float(_)
        | Kind::Var(_)
        | Kind::Local(_)
        | Kind::Input
        | Kind::Inputs
        | Kind::Lines
        | Kind::Dsv { .. } => {}
        Kind::VecLit(items) => items.iter().for_each(|i| calls_in(i, out)),
        Kind::RecordLit { fields } => fields.iter().for_each(|(_, v)| calls_in(v, out)),
        Kind::EnumLit { payload, .. } => {
            if let Some(p) = payload {
                calls_in(p, out);
            }
        }
        Kind::Call { .. } => unreachable!("handled above"),
        Kind::Concat(l, r) => {
            calls_in(l, out);
            calls_in(r, out);
        }
        Kind::Not(base) => calls_in(base, out),
        Kind::Arith { lhs, rhs, .. }
        | Kind::Compare { lhs, rhs, .. }
        | Kind::Logic { lhs, rhs, .. } => {
            calls_in(lhs, out);
            calls_in(rhs, out);
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
            calls_in(value, out);
            calls_in(body, out);
        }
        Kind::Select { source, pred, .. } => {
            calls_in(source, out);
            calls_in(pred, out);
        }
        Kind::Field { base, .. } | Kind::Unwrap { base } => calls_in(base, out),
        Kind::Builtin { arg, .. } => calls_in(arg, out),
        Kind::Index { base, index, .. } => {
            calls_in(base, out);
            calls_in(index, out);
        }
        Kind::Slice {
            base, start, end, ..
        } => {
            calls_in(base, out);
            if let Some(s) = start {
                calls_in(s, out);
            }
            if let Some(e) = end {
                calls_in(e, out);
            }
        }
        Kind::Match { subject, arms, .. } => {
            calls_in(subject, out);
            for a in arms {
                if let Some(g) = &a.guard {
                    calls_in(g, out);
                }
                calls_in(&a.body, out);
            }
        }
    }
}
