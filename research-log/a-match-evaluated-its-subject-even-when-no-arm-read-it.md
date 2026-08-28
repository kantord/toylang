---
type: Lesson
calendar:
  - 2026-08-28
title: A match evaluated its subject even when no arm read it
description: Native's match codegen called self.expr(subject) unconditionally, forcing a whole-record read out of a struct-of-arrays cursor for a pure guard chain that never touches the subject value, only its fields.
tags:
  - native
  - llvm
  - struct-of-arrays
  - correctness
timestamp: 2026-08-28T00:00:00Z
---

Issue #40: `[{valid: ..., readings: [..]}] | map(.valid -> .readings[0]! or any() -> 0)` refused
to compile on native with "the native backend cannot compile using a Vec element as a whole value
yet" -- the error [SoA is cheap until something wants a whole element](soa-is-cheap-until-something-wants-a-whole-element.md)
predicted would appear the day something asked for one. Nothing in the program does: `.valid` and
`.readings[0]` are both ordinary field reads through the cursor, which already worked (a plain
`map(.readings[0]!)`, no guard chain, compiled fine before this fix).

The guard chain is `Kind::Match` with every arm's `variant` set to `None`. Reading `match_arm`
shows the subject value is used in exactly two places, both gated on a variant being present: the
tag read (`rec_get(subj, 0)`, only when some arm has a variant) and a payload read (only inside an
arm that has one, which requires a variant). A pure guard chain has neither, so the subject's
*value* is never consulted anywhere in the arms -- only expressions nested in a guard or a body
read fields off it, through the already-working cursor path.

The codegen did not know that. `let subj = self.expr(subject)?;` ran before the loop over arms,
unconditionally, on every match regardless of whether any arm would end up using `subj`. For an
enum subject that costs nothing extra. For `.` bound to a struct-of-arrays cursor inside `map` or
`select`, `self.expr` on a bare `Local` pointing at a cursor is exactly the unsupported path, so
the eager read failed before the loop ever got a chance to not need it.

The fix makes the read conditional on the same test that already gated the tag: `needs_subject =
arms.iter().any(|a| a.variant.is_some())`. `subj` becomes `Option<BasicValueEnum>`, threaded
through `match_arm`, unwrapped with `.ok_or(...)` in the one place (`arm.payload`) that can only
be reached when `needs_subject` was true.

The general shape: a value can be *computed* without being *used*, and an optimisation that only
special-cases the read path (field access on a cursor) does nothing for a caller that
materialises the whole value up front regardless of whether it ends up reading from it. The
special case has to sit at the point where the value is actually consumed, not be assumed to
cover every route that happens to reach the same node. [One invariant, three independent
construction sites](one-invariant-three-independent-construction-sites.md) found the same shape
on the write side; this is the read side's version, and unlike that note's three sites this one
was a single call that simply ran too early rather than three call sites each getting their own
logic wrong.

Open: field access through a cursor is special-cased inside `field()` by pattern-matching
`Kind::Local(id)` as the immediate base of a `Field` node. Any future codegen path that calls
`self.expr()` on a bare cursor-bound local for a reason other than "I am about to read a field off
it" will hit the same wall this one did. There is no mechanism that catches that class of bug
before a program exercises it.
