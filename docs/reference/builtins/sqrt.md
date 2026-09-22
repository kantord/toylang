# sqrt

`sqrt: Float -> Float`: the square root of its argument, the one transcendental in the
language. The ruling (2026-09-20) fixes its edges: `sqrt` of a negative is `NaN`, the IEEE
answer, rather than a stop the way an `Int`'s `1 / 0` is. There is no `Float -> Int` in the
language, so no floor or round rides along; a rounding builtin would be its own row.

Lands on every backend. `sqrt(2.0)` prints `1.4142135623730951` on all seven.

Python's `math.sqrt` raises on a negative input rather than returning NaN, so its emitter arm
guards that case; every other backend's own sqrt already agrees with the ruling natively (jq's
own `sqrt` gives `nan`, which prints through the same `tl_show_float` path a `0.0 / 0.0` does,
and native's arm is the `llvm.sqrt.f64` intrinsic, which lowers to the target's libm `sqrt` and
so gives the same IEEE 754 NaN under the hood).
