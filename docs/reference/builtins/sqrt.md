# sqrt

`sqrt: Float -> Float`: the square root of its argument, the one transcendental in the
language. The ruling (2026-09-20) fixes its edges: `sqrt` of a negative is `NaN`, the IEEE
answer, rather than a stop the way an `Int`'s `1 / 0` is. There is no `Float -> Int` in the
language, so no floor or round rides along; a rounding builtin would be its own row.

Landed so far on Go, Rust, Python, JS, Lua, and jq (`sqrt(2.0)` prints `1.4142135623730951` on
each). Native (the LLVM backend) has no arm for it yet, so a program that reaches it is
refused before anything is emitted, the same way `pipe_through` is on a backend that has not
landed it:

```
`sqrt` has no native backend yet; today it runs on go and rust and py and js and lua and jq
```

Python's `math.sqrt` raises on a negative input rather than returning NaN, so its emitter arm
guards that case; every other backend's own sqrt already agrees with the ruling natively (jq's
own `sqrt` gives `nan`, which prints through the same `tl_show_float` path a `0.0 / 0.0` does).

Because one backend still refuses, no reference page can show `sqrt(2.0)` running against all
of them, and the tag-corpus row for it is a debt held in `tests/tag_coverage.rs`
(`builtin.sqrt`) until the last backend lands it.
