//! The v1 mutation rule, the part of it that is about the tree and not about any target: may a
//! Vec/Str binding be consumed in place at its one use, instead of copied?
//!
//! A backend still owns everything about *how* -- Rust moves the value into an `_owned` helper,
//! where another target would reuse a table -- and, for a target whose values can be shared, the
//! question of whether the binding's value is shared with anything else. This module only
//! answers the counting half, which is the same on every target, and, for targets whose Vecs are
//! reference types, the aliasing half (`owned_locals`).

use std::collections::{HashMap, HashSet};

use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
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
    let scan = scan(body, local);
    scan.count == 1 && scan.consuming && !scan.in_rerun_body
}

/// `single_consuming_use` without the "consuming" half: one use, run once.
fn single_use(body: &Tir, local: LocalId) -> bool {
    let scan = scan(body, local);
    scan.count == 1 && !scan.in_rerun_body
}

struct Scan {
    count: usize,
    consuming: bool,
    in_rerun_body: bool,
}

fn scan(body: &Tir, local: LocalId) -> Scan {
    let mut scan = Scan {
        count: 0,
        consuming: false,
        in_rerun_body: false,
    };
    tir::each_node(body, &mut |node| {
        match &node.kind {
            Kind::Map { body: inner, .. }
            | Kind::SortBy { body: inner, .. }
            | Kind::MaxBy { body: inner, .. }
            | Kind::Closure { body: inner, .. }
            | Kind::Select { pred: inner, .. } => {
                scan.in_rerun_body |= uses(inner, local) > 0;
            }
            Kind::Local(id) if *id == local => scan.count += 1,
            Kind::Builtin { which, arg } => {
                if matches!(which, Builtin::Sort | Builtin::Reverse | Builtin::Flatten)
                    && matches!(&arg.kind, Kind::Local(id) if *id == local)
                {
                    scan.consuming = true;
                }
            }
            // A Vec/Str `+`-concat consumes either operand.
            Kind::Concat(l, r) => {
                if matches!(node.ty, Type::Vec(_) | Type::Str)
                    && (matches!(&l.kind, Kind::Local(id) if *id == local)
                        || matches!(&r.kind, Kind::Local(id) if *id == local))
                {
                    scan.consuming = true;
                }
            }
            _ => {}
        }
    });
    scan
}

fn uses(t: &Tir, local: LocalId) -> usize {
    let mut n = 0;
    tir::each_node(t, &mut |node| {
        n += usize::from(matches!(&node.kind, Kind::Local(id) if *id == local));
    });
    n
}

/// How much of a value is known to be reachable only through the value itself.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
enum Fresh {
    /// Could be reachable by another name: a parameter, a record field, a Vec element, an input,
    /// a call's result, anything not listed below.
    Shared,
    /// The top-level container was built by this expression and nothing else refers to it.
    Top,
    /// `Top`, and every container directly inside it (a Vec's elements, an enum's payload) was
    /// built here too, and appears once.
    Elems,
}

type Env = HashMap<LocalId, Fresh>;

/// The Vec/Str locals a target whose containers are shared by reference may mutate in place
/// where they are consumed, because no other name can observe the change.
///
/// `single_consuming_use` is enough on Rust, where an owned value is unique by construction. It
/// is not enough on a target where binding, passing, storing and capturing all share the one
/// container: `let ys = xs` makes `ys` the only reader of nothing, and `[xs]` stores the very
/// table `xs` names. So a local also has to start out fresh -- built by a producer that returns a
/// new container -- and everything on the way to its one use has to preserve that:
///
/// - the value bound is a list of producers only (see `freshness`); a parameter, a `Field` or
///   `Index` read, an input, a call's result, a match arm's result are all refused, since each
///   can hand back a container something else holds. Calls are refused for the same reason the
///   function-boundary ruling gives.
/// - a `Map` element or a `Match` payload starts out fresh only when the source's elements were
///   built in the same expression, once each (`Fresh::Elems`).
/// - the one use is not inside a body that runs more than once, and not inside a match subject
///   that is not a plain local, which a target may write once per arm.
///
/// Str locals are never listed: their consuming use is a concatenation that builds a new string
/// on any target that has immutable ones, so there is no copy to save and nothing to mutate.
pub fn owned_locals(program: &Program) -> HashSet<LocalId> {
    let mut owned = HashSet::new();
    let mut env = Env::new();
    let roots = program.funcs.iter().map(|f| &f.body).chain([&program.body]);
    for root in roots {
        tir::each_node(root, &mut |node| match &node.kind {
            Kind::Bind { local, value, body } => {
                let fresh = freshness(value, &mut env);
                let once = once(body, *local);
                if fresh >= Fresh::Top && once && single_consuming_use(body, *local) {
                    owned.insert(*local);
                }
                if fresh > Fresh::Shared && once {
                    env.insert(*local, fresh);
                }
            }
            Kind::Map {
                source,
                param,
                body,
            } => elements_of(
                freshness(source, &mut env),
                *param,
                body,
                &mut owned,
                &mut env,
            ),
            Kind::Match { subject, arms, .. } => {
                let fresh = freshness(subject, &mut env);
                for arm in arms {
                    if let Some(payload) = arm.payload {
                        elements_of(fresh, payload, &arm.body, &mut owned, &mut env);
                    }
                }
            }
            _ => {}
        });
    }
    owned
}

/// One use, run once, and not somewhere the emitter may write more than once.
fn once(body: &Tir, local: LocalId) -> bool {
    single_use(body, local) && !in_duplicated_subject(body, local)
}

/// `param` is an element of a container whose freshness is `source`: fresh itself only if the
/// container's elements were built with it.
fn elements_of(
    source: Fresh,
    param: LocalId,
    body: &Tir,
    owned: &mut HashSet<LocalId>,
    env: &mut Env,
) {
    if source < Fresh::Elems || !once(body, param) {
        return;
    }
    if single_consuming_use(body, param) {
        owned.insert(param);
    }
    env.insert(param, Fresh::Top);
}

/// A `Match` reorder pass can put an arbitrary expression in the subject slot, and a target that
/// tests the subject once per arm then evaluates it once per arm.
fn in_duplicated_subject(body: &Tir, local: LocalId) -> bool {
    let mut found = false;
    tir::each_node(body, &mut |node| {
        if let Kind::Match { subject, .. } = &node.kind {
            found |= !matches!(subject.kind, Kind::Local(_)) && uses(subject, local) > 0;
        }
    });
    found
}

/// Whether evaluating `t` yields a container built by `t` itself. Only producers that return a
/// new container whatever their input is are listed; everything else is `Shared`, which is the
/// answer that never causes a mutation.
///
/// `env` holds the locals already known to be fresh and read once. Reading one hands its
/// container on, still unshared.
fn freshness(t: &Tir, env: &mut Env) -> Fresh {
    match &t.kind {
        Kind::VecLit(items) => {
            if items.iter().all(|i| freshness(i, env) >= Fresh::Top) {
                Fresh::Elems
            } else {
                Fresh::Top
            }
        }
        Kind::EnumLit {
            payload: Some(p), ..
        } => {
            if freshness(p, env) >= Fresh::Top {
                Fresh::Elems
            } else {
                Fresh::Top
            }
        }
        Kind::Builtin { which, arg } => match which {
            Builtin::Range
            | Builtin::Sort
            | Builtin::Reverse
            | Builtin::Flatten
            | Builtin::Chars => Fresh::Top,
            Builtin::Transpose => Fresh::Elems,
            // A materialized stream is the very container that produced it.
            Builtin::Collect => freshness(arg, env),
            _ => Fresh::Shared,
        },
        Kind::Concat(..) if matches!(t.ty, Type::Vec(_)) => Fresh::Top,
        Kind::SortBy { .. } => Fresh::Top,
        Kind::Map { body, .. } => {
            if freshness(body, env) >= Fresh::Top {
                Fresh::Elems
            } else {
                Fresh::Top
            }
        }
        Kind::Bind { local, value, body } => {
            let fresh = freshness(value, env);
            if fresh > Fresh::Shared && once(body, *local) {
                env.insert(*local, fresh);
            }
            freshness(body, env)
        }
        Kind::Local(id) => env.get(id).copied().unwrap_or(Fresh::Shared),
        _ => Fresh::Shared,
    }
}
