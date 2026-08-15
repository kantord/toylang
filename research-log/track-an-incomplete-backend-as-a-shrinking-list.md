---
type: Technique
calendar:
  - 2026-08-11
title: Track an incomplete backend as a shrinking list
description: A partial backend cannot join an agreement harness without softening it, and leaving it out is a silent skip, so the resolution is to snapshot what it cannot do and require that list to shrink.
tags:
  - testing
  - backends
  - prototype-1-5
timestamp: 2026-08-11T00:00:00Z
---

The agreement harness says every backend must produce the same output for every corpus program,
and it treats a backend that cannot run as a failure rather than something to pass over. The
native backend arrived able to compile one program out of nineteen. Both available moves are
wrong:

- **Add it to the harness.** Eighteen failures, a permanently red suite, and within a day nobody
  reads the output. A red suite that is expected to be red is a suite that has stopped working.
- **Soften the harness** so unsupported constructs are tolerated. That is exactly the silent skip
  the harness exists to prevent, and it would apply forever rather than during the gap.
- **Leave it out.** Honest about the harness, but native then has no coverage at all and nothing
  notices when it stops working.

The resolution is a fourth thing: leave it out of the harness, and separately assert over the
whole corpus that each program is *either* compiled natively and checked against another backend,
*or* named in a snapshot with the reason it could not be. Nothing is skipped, because every
program is accounted for in one list or the other.

What makes it work is that the snapshot is versioned. The gap is not a fact somebody remembers,
it is a file in the repo that a diff shows moving:

```
compiles natively (1):
hello

not yet (18):
adults: the native backend cannot compile functions yet
concat: the native backend cannot compile any expression but a string literal yet
...
```

Two properties fall out for free. The list cannot quietly grow, since a regression that breaks a
construct shows up as a line moving from one section to the other. And progress is visible as a
diff rather than as a claim, so "step 5 added functions" is checkable rather than asserted.

The general shape: when a component is deliberately incomplete, the incompleteness itself is the
thing to put under test. An exclusion that lives in someone's head is indistinguishable from a
bug; an exclusion that lives in a snapshot is a to-do list the test suite maintains.

This is the same instinct as
[a test that cannot fail is worse than no test](a-test-that-cannot-fail-is-worse-than-no-test.md),
applied to coverage rather than to assertions: there the question was whether a test could ever
go red, here it is whether an absence would ever be noticed.
