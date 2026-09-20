# sqrt

`sqrt: Float -> Float`: the square root of its argument, the one transcendental in the
language. The ruling (2026-09-20) fixes its edges: `sqrt` of a negative is `NaN`, the IEEE
answer, rather than a stop the way an `Int`'s `1 / 0` is. There is no `Float -> Int` in the
language, so no floor or round rides along; a rounding builtin would be its own row.

This row lands the builtin in the front end only: it type-checks, and the refusal happens in
one place, [`refuse_unbuilt`](../../../src/backend_support.rs), before any emitter runs. No
backend has an arm for it yet, so a program that reached here is refused everywhere with the
same message a program using `pipe_through` on a backend that has not landed it gets:

```
`sqrt` has no rust backend yet; today it runs on
```

Because every backend refuses, no reference page can show `sqrt(2.0)` running, and the
tag-corpus row for it is a debt held in `tests/tag_coverage.rs` (`builtin.sqrt`) until a
built-on backend gives the refusal a backend to name.
