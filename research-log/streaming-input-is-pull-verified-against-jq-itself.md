---
type: Note
calendar:
  - 2026-08-25
title: Streaming input is pull, verified against jq itself
description: Every mature abstraction that stops a producer from outrunning its consumer turns out to be pull, including jq -- verified empirically -- which decided the model for toylang's first streaming primitive with no new machinery needed to get backpressure.
tags:
  - streams
  - jq
  - design
timestamp: 2026-08-25T00:00:00Z
---

The question was push or pull for `lines`, the first construct that reads stdin incrementally
rather than all at once. The candidates worth comparing: Rust's `Iterator`, Python's generators,
JavaScript's async iterators, Node's raw `Readable` (push), and jq itself, since it is a sibling
backend rather than a precedent from elsewhere.

`limit(3; range(100000000000))` in real jq returns three values instantly rather than attempting
to materialise an impossible sequence. That is empirical, not read from documentation, and it
settles the question for jq specifically: its evaluation is demand-driven, so a `lines` design
modelled on it should be too.

The general pattern behind that single fact: every abstraction whose job is specifically not
letting a producer outrun its consumer is pull. Rust's `Iterator`, Python's generators,
JavaScript's async iterators (the layer people actually write against for real stream
consumption) are all pull, and get backpressure as a free consequence of the calling convention
-- nothing runs until asked. Push-based designs need a second, explicit protocol bolted on to
recover the same property: Node's raw `Readable` in flowing mode is the cautionary case, a
well-documented backpressure footgun that is exactly why `pause`/`resume` exist and why the
ecosystem migrated to async iterators; Reactive Streams' `request(n)` demand signal is the same
fix, formalised into four cooperating interfaces.

Two costs, decided rather than discovered as a gap later: fan-out (a pulled stream used in two
places) is not supported, and neither is overlap between pipeline stages, since a pull chain
never runs ahead of the consumer. Both are named non-goals up front. What survives regardless is
the cross-process overlap that `grep foo | wc -l` gets from the kernel scheduling two real
processes, which is a property of not pre-reading stdin into Rust before a subprocess backend
runs, not something push buys that pull does not.

The restricted operator set this cut settled on -- a linear chain, `lines` collapsing through
`collect` -- turned out to matter for a second reason: it compiles to one ordinary loop on every
backend including native, which has no coroutine or event-loop runtime to build one on top of.
Pull plus "no branching fan-out" needed zero new runtime machinery anywhere, which a general
push-based design, or a pull design that allowed multiple consumers, would not have gotten for
free.

See [a minimal cut of streaming input](../draft.md#decided-a-minimal-cut-of-streaming-input-pull-based-one-new-keyword)
for what got built on top of this, and
[a sixth instance of the backend having rules the checker does not](a-sixth-backend-rule-the-checker-did-not-know.md)
for what verifying each backend's own line-splitting against this model turned up.
