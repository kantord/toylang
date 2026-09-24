//! The v1 mutation rule, the part of it that is about the tree and not about any target: may a
//! Vec/Str binding be consumed in place at its one use, instead of copied?
//!
//! A backend still owns everything about *how* -- Rust moves the value into an `_owned` helper,
//! where another target would reuse a table -- and, for a target whose values can be shared, the
//! question of whether the binding's value is shared with anything else. This module only
//! answers the counting half, which is the same on every target.

use crate::tir::{self, Builtin, Kind, LocalId, Tir};
use crate::ty::Type;

/// Does `body` hold exactly one use of `local`, and is that one use the argument to a consuming
/// builtin (Sort/Reverse/Flatten, or a Vec/Str `+`-concat)?
///
/// The walk is a plain occurrence count over the body's tree (`tir::each_node`). It works for
/// every binding form because each form's parameter is a fresh, unique `LocalId`: a `Bind`'s
/// local, a `Map`/`Select`/`OptMap`'s param, or a `Match` arm's payload are all just a `LocalId`
/// to count in their body. A `Call` node holds only its argument, never a callee's body, so a
/// use reached through a called function is simply not in this tree and cannot qualify -- the
/// mutation-function-boundary ruling falls out of that shape rather than needing a separate
/// check.
///
/// One use in the source is not one use at run time when it sits inside a body that runs more
/// than once (a `Map`/`Select`/`SortBy`/`MaxBy` body, a closure) while the binding lives outside
/// it: `let xs = ...; ys | map(reverse(xs))` reverses `xs` once per element. Such a use never
/// qualifies. A binding that is the loop's own parameter is unaffected, since the walk starts at
/// the loop's body and each iteration binds it afresh.
pub fn single_consuming_use(body: &Tir, local: LocalId) -> bool {
    let mut count = 0usize;
    let mut consuming = false;
    let mut in_rerun_body = false;
    tir::each_node(body, &mut |node| {
        match &node.kind {
            Kind::Map { body: inner, .. }
            | Kind::SortBy { body: inner, .. }
            | Kind::MaxBy { body: inner, .. }
            | Kind::Closure { body: inner, .. }
            | Kind::Select { pred: inner, .. } => {
                in_rerun_body |= uses(inner, local) > 0;
            }
            Kind::Local(id) if *id == local => count += 1,
            Kind::Builtin { which, arg } => {
                if matches!(which, Builtin::Sort | Builtin::Reverse | Builtin::Flatten)
                    && matches!(&arg.kind, Kind::Local(id) if *id == local)
                {
                    consuming = true;
                }
            }
            // A Vec/Str `+`-concat consumes either operand.
            Kind::Concat(l, r) => {
                if matches!(node.ty, Type::Vec(_) | Type::Str)
                    && (matches!(&l.kind, Kind::Local(id) if *id == local)
                        || matches!(&r.kind, Kind::Local(id) if *id == local))
                {
                    consuming = true;
                }
            }
            _ => {}
        }
    });
    count == 1 && consuming && !in_rerun_body
}

fn uses(t: &Tir, local: LocalId) -> usize {
    let mut n = 0;
    tir::each_node(t, &mut |node| {
        n += usize::from(matches!(&node.kind, Kind::Local(id) if *id == local))
    });
    n
}
