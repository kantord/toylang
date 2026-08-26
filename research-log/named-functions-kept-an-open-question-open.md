---
type: Lesson
calendar:
  - 2026-08-26
title: Named functions kept an open question open
description: extent, concat, and tail were spelled as named builtins rather than as an operator overload or a new Spec, specifically so adding them would not force an answer to Q2, which is still open.
tags:
  - design-process
  - vec
  - primitives
timestamp: 2026-08-26T00:00:00Z
---

Three primitives were needed to make bitwise cyclic tag (a Turing-completeness witness) writable:
a Vec's length, a way to grow one, and a way to drop its first element. jq's own spellings for
the last two are `+` on two arrays and `.[1:]` slicing. Neither was used.

`src/check.rs`'s `binary()` already refuses any operator on a Vec, with a comment naming the
reason: [Q2](../draft.md#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit)
is open, so an operator over a Vec is rejected rather than being silently given broadcast or zip
semantics. Overloading `+` for Vec concatenation would have been a real answer to a real question
-- jq's answer, specifically -- shipped as a side effect of an unrelated feature, in the same
commit that also renamed `Kind::Map`'s tag to avoid colliding with CONTEXT.md's reserved `Map`
type. `concat(vv: Vec<Vec<T>>) -> Vec<T>` gets the same operation jq's `+` gives arrays (jq's own
`add` is the closer match, in fact: flattening a list of lists rather than joining exactly two)
without touching the operator table at all, or deciding what `+` means for two Vecs in general.

Same reasoning for `tail`. jq's `.[1:]` would have meant extending the `[]`/`[i]` bracket-spec
system with two-sided slicing -- negative indices, out-of-range clamping, and where it sits
against Keep/Narrow/Collapse all becoming decisions made in passing, for a feature that only
needed to drop one element. `tail(v: Vec<T>) -> Opt<Vec<T>>` is a function call instead, returning
`None` on an empty Vec the same way `Index` already turns reaching past what's there into `Opt`.

`extent` needed no such care -- a dense Vec already tracks its own length, so reading it out is
metadata access, not an operation with semantics to argue about. It is also not spelled `length`:
CONTEXT.md's glossary already reserves that word's absence (`_Avoid: length, size, cardinality_`)
for "Extent," so the builtin took the glossary's name instead of adding a second one.

The general lesson: **a named function is a smaller commitment than an operator overload or a new
syntax form, because it cannot be reached except by writing its name.** `+` and `[]` are already
reachable from every expression that types even close to right, so extending what they mean
extends what every existing expression might now do. A new name only ever means what it says.
Where a language already has an open question sitting on a piece of syntax, adding a feature
through a new name rather than through that syntax is how the feature ships without being read,
later, as the answer to a question nobody actually decided.

The native implementation (`tl_vec_tail`, `tl_vec_concat` in `runtime/toylang.c`) is a fourth
place that has to respect the struct-of-arrays column invariant
[three other construction sites](one-invariant-three-independent-construction-sites.md) each
violated independently before being fixed. Both loop over `ncols` generically rather than
assuming column 0, so this one did not need its own bug first.

Open: `tail`'s Opt-wrapping and `concat`'s flatten shape are one reasonable design, not the only
one. A version of this language that eventually answers Q2 in favor of zip or cartesian semantics
might make `tail`/`concat`/`extent` feel like the wrong primitives to have hand-picked -- the
point made here is only that picking them this way did not foreclose that later answer.
