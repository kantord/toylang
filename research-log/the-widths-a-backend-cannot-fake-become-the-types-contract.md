---
type: Lesson
calendar:
  - 2026-08-30
title: The widths a backend cannot fake become the type's contract
description: Int64 landed with three different backend stories -- native width, a second representation, and an honest precision boundary -- and the boundary had to move into the documented contract because no emulation trick could reassemble 64-bit wrapping from doubles.
tags:
  - backends
  - int64
  - arithmetic
  - jq
timestamp: 2026-08-30T00:00:00Z
---

32-bit wrapping was emulable on every backend, which let ADR 0006 make one promise everywhere:
jq's worst case, multiplication, split into 16-bit halves whose partial products each fit a
double, and the harness could pin `-2147483648 * -1` across all seven targets. Int64
(kantord/toylang#83) is the width where that stops being available. There is no split that
reassembles a 64-bit product -- or a mod-2^64 wrap -- from pieces a double can hold.

So the backends divide into three honest stories rather than one emulation:

- **Native width.** Lua, Go, Rust, LLVM, and Python (whose unbounded integers make the wrap one
  modulo). Lua is the surprise: its integers are 64-bit already, so Int64's `+`, `-`, `*` are
  bare operators while Int's need `tl_i32` fixups -- the wider type is the cheaper one there.
- **A second representation.** JavaScript carries Int64 as BigInt behind `BigInt.asIntN(64, ...)`,
  which is exactly the cost draft.md's carrying-width correction predicted: 53 bits was the
  ceiling for staying on doubles, and going past it means two numeric representations in one
  emitted program.
- **A documented boundary.** jq computes in doubles, exact within +/-2^53 and wrong past it.
  The alternative -- refusing Int64 programs on jq -- would have made every Int64 corpus case
  unwritable, since a case must either run on all seven backends or be refused by all seven.

The consequence for testing is structural, not incidental: the corpus can only pin behaviour
all seven backends share, so the 2^63 wrapping edges (`MAX + 1`, `MIN / -1`) moved to
tests/int64.rs, pinned across six backends with jq's divergent answer asserted beside them
rather than skipped. The boundary is part of the type's reference page for the same reason.
A promise the implementation cannot keep everywhere has to surface in the contract, or the
agreement harness quietly stops meaning what it says.

One knock-on defect worth remembering: the Rust backend generated a JSON parser for every
record type the program mentions, not just the ones reachable from `input`. Harmless while
every type was parseable; the first deliberately unparseable type (Int64, whose wire codec is
undecided) made those spurious parsers panic. The fix scopes parser generation to the types
stdin can actually deliver -- the general shape being that "generate for everything, it's
harmless" holds only until some type is unsupported on purpose.
