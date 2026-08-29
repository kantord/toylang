---
type: Lesson
calendar:
  - 2026-08-26
  - 2026-08-29
title: A map body cannot infer from what consumes it
description: expect() threads an expected type into exactly one form, Expr::Input; a map body is always synth'd bottom-up, so a polymorphic builtin like a hypothetical parse(s) -> T has nothing to resolve T against inside map(parse(.)), the same hole an empty [] literal falls into.
tags:
  - type-inference
  - checker
  - design-process
timestamp: 2026-08-26T00:00:00Z
---

Adding `inputs` (every remaining JSON value on stdin, eagerly collected into `Vec<T>`) started as
a more general idea: a `parse(s: Str) -> T` builtin, polymorphic like `extent`/`tail`/`concat`,
composing as `collect(lines) | map(parse(.))`. More reusable -- it would also parse a JSON string
sitting in an ordinary field -- and it would have needed no third stdin-exclusivity mode at all.

It does not work with the checker as it stands, and the reason generalizes past this one feature.
`expect(ctx, expr, want)` in `src/check.rs` special-cases exactly one expression form,
`Expr::Input`: everything else falls through to `synth`, which infers bottom-up with no `want` in
scope at all. `map`'s own body is *always* reached through plain `synth` -- `Expr::Call`'s
`func == "map"` arm does `synth(&ctx.with(...), arg)` unconditionally, with no path for a `want`
supplied to the outer `map` expression to reach the body being checked inside it. So `parse(.)`
inside `map(parse(.))` has no expected type to resolve `T` against, for the identical structural
reason a bare `[]` literal can never be typed even as a function's declared return value: nothing
threads an expected type more than one level deep into the expression tree.

The general form: **`expect()`'s bidirectional inference is one level deep, not recursive.** It
answers "does this exact expression match what's wanted" for a small, hand-picked list of forms
(`Expr::Input` today); it does not propagate that answer into sub-expressions that themselves
have no other way to learn what's wanted of them. Any future feature that wants to be polymorphic
and used inside `map`, `select`, or `Cond`'s branches -- another `Input`-shaped position, a
generic collection builder, anything needing `want` to resolve a type variable -- will hit this
same wall, not a new one.

What made `inputs` a fine substitute rather than a downgrade: it does not need `map` at all. As a
bare keyword checked directly by `expect()`, the same mechanism `input` already uses, it sits in
exactly the one position the checker already threads a type into. The general `parse` builtin is
still worth wanting -- it would remain more reusable than a dedicated keyword -- but it is real,
separate work (propagating an expected type through `Map` at minimum, and probably `Select` and
`Cond` for the same reason), not something to build as a side effect of reading NDJSON.

This is the same shape as
[one invariant, three independent construction sites](one-invariant-three-independent-construction-sites.md):
a single missing piece of machinery (there, per-column struct-of-arrays spreading; here, deep
type-directed inference) that every future feature reaching for the same corner will rediscover
independently until the machinery itself exists.

Open: whether the fix is "thread `want` through `Map`/`Select`/`Cond` specifically" or something
more general (full bidirectional inference, à la Hindley-Milner with expected-type propagation).
The narrower fix is smaller and answers the case actually hit; the general one would stop this
from being rediscovered a third time.

Closed 2026-08-29: the answer was the narrower fix, and it was enough. The type-flow rework
(`plans/type-flow.md`) made `expect` recursive over the forms that denote shapes -- pipes,
record and Vec literals, `map` bodies, conditionals, total match chains -- with no unification
variables anywhere; the position's declared type is the only source. `map(parse(.))`-shaped
bodies now check against the expected element, which is what the `parse` design was waiting on.
The class this wall guarded is
[checked-only forms are a class, not a lambda rule](checked-only-forms-are-a-class-not-a-lambda-rule.md),
whose closing section lists what pushed where.
