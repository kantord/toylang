---
type: Note
calendar:
  - 2026-08-10
title: Losing jaq's corpus means building the agreement harness
description: Dropping the jaq fork also dropped a ready-made conformance suite, so cross-backend agreement testing stops being inherited and becomes something toylang has to build for itself.
tags:
  - testing
  - backends
  - jq
timestamp: 2026-08-10T00:00:00Z
---

The decision to treat jq as a reference rather than a conformance target removed the reason to
fork jaq. That was the right call for the front end, and it has a cost that was not part of the
argument at the time.

jaq carries roughly 640 assertions. Inheriting them would have given more than jq compatibility:
it would have given a large corpus of programs with known-correct outputs, runnable against every
backend, which is exactly the shape of test that catches a backend disagreeing with the others.
That harness was going to arrive for free. Now it does not.

This matters more than it would for a single-target compiler, because toylang plans three
backends with genuinely different execution models, and
[each one has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md).
A bug that only appears on one target is invisible to any test that runs only on another.

What survives from jaq is still worth having: it remains readable as a reference implementation,
and its measured behaviours are usable as expectations wherever toylang deliberately agrees with
jq. What has to be built is the mechanism -- a corpus of programs with expected outputs, run
across every backend, with disagreement between backends being itself a failure rather than
something only visible when both are compared by hand.

Open: whether the corpus is written by hand as the language grows, or generated. Nothing yet
depends on the answer, since prototype 1 has one backend and disagreement is not expressible.
