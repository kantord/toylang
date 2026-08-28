---
type: Lesson
calendar:
  - 2026-08-28
title: Codepoint order is not UTF-16 order
description: JavaScript's native `<` on strings compares UTF-16 code units, which disagrees with the other six backends' codepoint order on any pair straddling a surrogate pair.
tags:
  - strings
  - backends
  - js
timestamp: 2026-08-28T00:00:00Z
---

`str_ordering.yaml` pinned `"ada" < "bo"` and nothing else, which is
[the kind of evidence a backend gives when nothing distinguishes it](a-backend-that-finds-nothing-is-evidence-only-if-it-is-different.md):
ASCII order is byte order on every representation, so agreement there proves nothing about the
axis where the backends actually differ. That axis is exactly the one the design draft already
named JavaScript as the target that would eventually force: JS is
[the target with a foreign string model](a-backend-that-finds-nothing-is-evidence-only-if-it-is-different.md#what-the-exercise-is-for)
this repo did not yet have, a UTF-16 string over five backends whose strings are UTF-8 bytes (Go,
Lua, Rust, native) or codepoint-addressed already (Python, jq).

**The concrete pair.** An astral character, U+1F600, encoded as the surrogate pair `0xD83D
0xDE00`, compared against a BMP character above the surrogate block, U+E000. By codepoint
value `0x1F600 > 0xE000`, so U+E000 sorts before U+1F600 under every codepoint comparison.
JavaScript's native `<` compares the first UTF-16 code unit of each operand instead: `0xD83D`
(the astral character's lead surrogate) against `0xE000`, and `0xD83D < 0xE000`, so native JS
said the opposite. Six backends agreed; JS alone flipped the answer, verified directly with
`tests/corpus/str_ordering_codepoint.yaml` before any fix landed.

**Why it stayed hidden.** Every ordering operator across every backend was one line --
`format!("({} {} {})", expr(lhs), op, expr(rhs))` -- because nothing before this exercised a
comparison where a backend's native operator and codepoint order could differ. The native/LLVM
backend was the one exception already, dispatching `Str` through a runtime `tl_str_cmp` (byte
order, which happens to equal codepoint order for valid UTF-8) rather than a native operator at
all, purely because LLVM IR has no polymorphic `<` to reach for. That accident of implementation
is what the other five backends had for free and JS did not.

**The fix.** `src/emit_js.rs` now special-cases `Str` ordering the same way LLVM already had to:
a `tl_str_cmp(a, b)` helper that walks both strings with the string iterator protocol (`for...of`
semantics, one codepoint per step, surrogate pairs consumed together) rather than indexing by
UTF-16 code unit, and the four ordering operators compare its result against zero instead of
using `<`/`<=`/`>`/`>=` directly. Equality (`===`/`!==`) needed no change: two strings with the
same codepoint sequence have the same UTF-16 code units regardless of which order comparison
they're compared under, so equality was never the axis at risk.

Open: no other operator or builtin reads a `Str` byte-by-byte or code-unit-by-unit anywhere in
the language today, so this was the only place JS's representation could leak through. If a
future feature indexes into a `Str` (there is none now -- see
[`Str` has no dimensions](../docs/reference/types/str.md)), the same UTF-16-vs-codepoint gap
would reopen there independently.

Found in the same session as
[unescapable control bytes are the crack in the re-serialization gate](unescapable-control-bytes-are-the-crack-in-the-reserialization-gate.md),
which is the other shape this kind of gap takes: a backend's own decoder having a rule nothing
before it had tested, rather than its own encoder disagreeing on a comparison.
