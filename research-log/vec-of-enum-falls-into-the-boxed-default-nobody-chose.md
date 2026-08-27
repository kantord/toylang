---
type: Note
calendar:
  - 2026-08-27
title: Vec of enum falls into the boxed default nobody chose
description: Boxing a scalar enum as a tag-plus-payload record made Vec<enum> run on native with zero added code, so step 4's open layout choice already has a de facto answer that no test pins and nobody decided.
tags:
  - layout
  - backends
  - enums
timestamp: 2026-08-27T00:00:00Z
---

Enum step 1 needed a scalar enum value on the native backend, and the cheapest representation
was a boxed two-slot record built with the runtime that already exists: slot 0 the variant's
declaration index, slot 1 the payload. One `tl_rec` allocation, no new C.

The unplanned consequence: `Vec<enum>` works too, with zero enum-aware code. The columns rule
says anything that is not a record gets one column, and a slot already holds a pointer, so
`[a, b{x: 1}, a]` compiles and prints `["a",{"b":{"x":1}},"a"]` on native -- verified by
running it, not assumed. Nothing in step 1 was supposed to touch this.

That is exactly option (b) of the choice `plans/enums.md` step 4 leaves open: boxed per-element
enum columns (simple, slower, a special case in a backend built on not having special cases)
versus Arrow-style tag buffer plus per-variant child columns (columnar, vectorisable, more
work). The boxed form now exists de facto, decided by a default rather than by anyone. No
corpus case pins it, deliberately, so step 4 is still free to choose (a) -- but whoever takes
step 4 should know the incumbent is already running, because an incumbent that works is harder
to displace than a blank page, and silence here would make the accident look like a decision.

Step 4 then examined the accident against `runtime/toylang.c` as it actually is, and adopted
it. What the reading showed: the runtime has exactly two column shapes, not one per type --
`tl_parse_vec` asks `is_record` and gives everything else one column of 8-byte slots, and a
`Str` element is already a boxed pointer per element. So "boxed enum columns" is not the
special case the plan's framing feared; it is the existing non-record default doing its job,
and the *columnar* dense union (a tag buffer plus per-variant child columns, Arrow's answer)
would be a third column shape -- new construction sites for the invariant
[one invariant, three independent construction sites](one-invariant-three-independent-construction-sites.md)
already shows is easy to violate. Nothing in the language yet projects per-variant data out of
a `Vec<enum>` (match branches per element; no operator is vectorizable over variants), so the
columnar layout's payoff has no customer. The gather boundary from
[SoA is cheap until something wants a whole element](soa-is-cheap-until-something-wants-a-whole-element.md)
frames the revisit trigger precisely: an enum element is a whole-element want by construction,
and the day an operation wants a single variant's data across a whole Vec -- a vectorizable
match, a per-variant projection -- is the day the dense union earns its construction sites.
The corpus now pins the boxed behaviour (enum elements travel through `Vec` on all seven
backends), so the choice is a decision with a witness rather than a default nobody looks at.
