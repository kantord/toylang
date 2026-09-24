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
pub fn single_consuming_use(body: &Tir, local: LocalId) -> bool {
    let mut count = 0usize;
    let mut consuming = false;
    tir::each_node(body, &mut |node| {
        match &node.kind {
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
    count == 1 && consuming
}
