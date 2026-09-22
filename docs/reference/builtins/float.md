# float

`float(x)`, `Int -> Float` (and, on a `Float`, returned unchanged): the exact conversion of
an integer to a `Float`, ruling 2026-09-20. The row also fills in the one line of
[ADR 0006](../../adr/0006-int-is-32-bits-and-wraps.md) that said `i64(x)` was the whole
conversion surface, because `float(x)` is a second conversion on it. It is the third
builtin conversion in the language, beside `i64(x)` (`Int -> Int64`,
[ADR 0010](../../adr/0010-int64-is-a-second-integer-type.md)) and `str(x)`; nothing converts
a `Float` back to an `Int`.

It is exact because `Int` is 32 bits and `Float` is 64
([ADR 0006](../../adr/0006-int-is-32-bits-and-wraps.md)), so every `Int` fits a `Float` with
nothing to round; a too-big value enters as a literal only where a `Float` is already
expected, never through the bridge.

Lands on every backend.

Go, Rust, and native are the backends where an `Int` and a `Float` are genuinely different
runtime types, so their arms cast (native's with `build_signed_int_to_float`); Python, JS,
Lua, and jq represent both the same way at runtime, so `float` is the identity there and only
changes which printer the static type picks.
