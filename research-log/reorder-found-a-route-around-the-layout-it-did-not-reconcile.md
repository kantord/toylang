---
type: Note
calendar:
  - 2026-08-29
title: Reorder found a route around the layout it did not reconcile
description: Record-reorder's Opt gap (#66) needed a way to rebuild a present value, not a way to match one, so it got its own node instead of waiting on the Match/Opt layout reconciliation the previous note left open.
tags:
  - backends
  - representation
  - enums
timestamp: 2026-08-29T00:00:00Z
---

[Borrowing the host's null borrowed its conflations](borrowing-the-hosts-null-borrowed-its-conflations.md)
left one thing open: native's Opt layout (null-or-one-slot-box) is a different shape from the
general enum's two-slot tag box, legal only because the checker refuses matching an Opt by
variant, with the reconciliation apparently waiting on the matcher-totality round to give Opt
ordinary match arms.

#66 needed to reach inside an Opt payload before that round runs, to fix the same hazard #64
fixed for Vec: a value crossing into a differently-ordered but equal `Opt<Record>` read the
native struct-of-arrays layout at the wrong column, silently. The instinct was to check whether
this forced the reconciliation early -- if `reorder_record` had to route an Opt through
`Match`/`EnumLit` the way it already does for every other enum, native's Match codegen would
need Opt-awareness (so would Rust's, whose `Match` lowers to `Ename::V_variant` patterns that do
not exist on `Option`; Go's `tlOpt[T]{ok, v}` has the same problem).

It didn't. Reorder does not need to ask "which variant is this" the way a match arm does; it
needs "if this is present, rebuild the payload; if absent, do nothing." That is a strictly
smaller question, and every backend already answers it somewhere -- `show`'s Opt branch, the
unwrap helper, `tl_at` -- as an ordinary presence branch. So the fix is a new Tir node,
`Kind::OptMap` (present-preserving, absent-preserving, never surface syntax), and each backend
implements it by copying the presence branch it already had rather than by teaching `Match`
Opt's three divergent representations. Every other enum still reorders through `Match`/`EnumLit`
unchanged, since only Opt keeps a representation of its own.

The general lesson: two problems that sound like the same problem ("reach inside an enum
payload") can have different minimum machinery, and the smaller one does not have to wait for
the bigger one's design round to finish. The open reconciliation from the previous note is still
open -- match-by-variant over an Opt subject is still refused, native's Match still cannot run
against Opt's layout -- #66 just didn't need it to be closed.
