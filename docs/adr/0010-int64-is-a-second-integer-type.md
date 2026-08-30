---
status: accepted
---

# Int64 is a second integer type: position-resolved literals, explicit i64(), wrapping

Recorded 2026-08-30. The surface was settled in the int64-surface grilling round of
2026-08-29 (kantord/toylang#83); this ADR records the decision next to
[ADR 0006](0006-int-is-32-bits-and-wraps.md), whose named cost -- millisecond timestamps do
not fit, "with a second integer type left as an open question" -- it closes.

Three decisions make the surface:

1. **Literals carry no suffix.** A literal that fits `Int` is `Int`; one that only fits
   `Int64` resolves wherever an `Int64` is expected, the `[]` rule applied to numbers. A
   too-big literal with no expectation stays an error rather than being guessed wide.
2. **No implicit widening.** Mixing the two types in one operator is an error naming
   `i64(x)`, the explicit conversion builtin and the whole conversion surface.
3. **Int64 wraps**, extending ADR 0006's rule to 64 bits: division truncates, a zero divisor
   is the only arithmetic failure, and `MIN / -1` is `MIN`.

The choice draft.md left open -- a 53-bit carrying type that stays on JavaScript's doubles,
or a real 64-bit type that costs JS a second numeric representation -- resolved to the
latter: JavaScript carries `Int64` as `BigInt`, wrapped through `BigInt.asIntN(64, ...)`.

jq cannot follow past 2^53. Its only number is the IEEE double, 32-bit wrapping was emulable
there only because every 16-bit partial product fit one, and no such split reassembles a
64-bit product. The strategy is a documented precision boundary rather than a refusal:
`Int64` arithmetic on jq is exact within +/-2^53 and honestly wrong past it, stated in
[the type's reference page](../reference/types/int64.md), with the corpus keeping every
seven-backend case inside the envelope and the wrapping edges pinned across the other six
backends in tests/int64.rs.

`input` refuses `Int64` anywhere in its shape for now: an `Int64` result prints fine, but
reading one back off the wire is codec design nobody has done (JavaScript's `JSON.parse`
returns doubles), so the read side waits rather than shipping wrong.
