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

This row lands the builtin in the front end only: it type-checks (on an `Int` or a `Float`),
and the refusal happens in one place, [`refuse_unbuilt`](../../../src/backend_support.rs),
before any emitter runs. No backend has an arm for it yet, so a program that reached here is
refused everywhere:

```
`float` has no rust backend yet; today it runs on
```

Because every backend refuses, no reference page can show `float(1) + 0.5` running, and the
tag-corpus row for it is a debt held in `tests/tag_coverage.rs` (`builtin.float`) until a
built-on backend gives the refusal a backend to name.
