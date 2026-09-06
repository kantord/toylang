# Float formatting per backend: what the build lanes will actually hit

Research for the three stalled float-build lanes (go/python/rust, gh:149). Each lane must
make its backend print a Float the way the JS reference (`String(number)`, ECMA-262
Number::toString) does, byte for byte, because the corpus agreement harness compares backend
output across targets. This is the shared knowledge those lanes were rediscovering.

Everything below was run, not read from a manual. Go 1.24.4, Python 3.13.5, Rust 1.98.1,
Node v20.19.2. Each claim carries the snippet that produced it. The one drift risk is the
toolchain: these are all reasonably current, but re-run the snippet before trusting it on a
newer major version.

The short version: round-trip precision is a non-problem on every backend (they all emit
shortest-round-trip digits, so format-then-parse recovers the exact bits), but **notation is
the whole game**. JS switches fixed/scientific at magnitudes that match *none* of the three
backends' default formatters. The digits are right; the layout is wrong, at different places
on every backend.

## The JS reference

The reference printer is `String(number)` in the JS backend (src/emit_js.rs, the
`Type::Float` arm of `show`), i.e. ECMA-262 Number::toString on a binary64. Verified:

```js
> [1e20, 1e21, 1e-6, 1e-7, 2.0, -0.0, 0.30000000000000004].map(String)
[ "100000000000000000000", "1e+21", "0.000001", "1e-7", "2", "0", "0.30000000000000004" ]
```

Two facts drive everything else:

- **The digits are shortest-round-trip.** `0.30000000000000004` is printed in full, not
  rounded. `String(0.1)` is `"0.1"`. This is what makes round-trip exact.
- **The notation rule is ECMA-262's.** Fixed notation when the decimal point falls within or
  just past the digit run (`k <= n <= 21`), scientific otherwise. Concretely: fixed for
  magnitudes from about `1e-6` up to just under `1e21`, scientific outside that band. The
  exponent has a sign but no zero-padding (`"1e+21"`, `"1e-7"`).

Special values are spelled as words: `NaN`, `Infinity`, `-Infinity`. Negative zero prints as
`"0"`. A value with no fractional part still drops the `.0` (`2.0` prints `"2"`).

Number() (the parse side) accepts leading/trailing whitespace, hex *integer* literals
(`Number("0x10")` is `16`), and the exact strings `NaN`, `Infinity`, `-Infinity` (case
sensitive, `+`/`-` prefix allowed on Infinity but not NaN). It rejects hex *float* literals
(`Number("0x1p2")` is `NaN`), underscores (`"1_000"` is `NaN`), and the bare words `Inf` /
`inf` / `infinity` (all `NaN`). Empty string parses to `0`. Out-of-range exponent overflows
to `Infinity`/`0` rather than erroring.

## The layout problem in one table

Each backend's native formatter produces shortest-round-trip digits but lays out the fixed /
scientific switch differently. The three switch points that matter, with the value that first
goes scientific:

| backend | formatter | first fixed->scientific, small | first fixed->scientific, large | 2.0 | -0.0 | exponent zero-pad |
|---|---|---|---|---|---|---|
| JS (reference) | `String(n)` | `1e-7` | `1e21` | `2` | `0` | no |
| Go | `FormatFloat 'g' -1` | `1e-5` | `1e7` | `2` | `-0` | no (2-digit min) |
| Python | `repr(f)` | `1e-5` | `1e16` | `2.0` | `-0.0` | yes (`1e-05`) |
| Rust | `Display` | never | never | `2` | `-0` | n/a |

That table is the whole finding. Every backend's digits match JS; every backend's layout
differs from JS at a different place. Go's band is the narrowest: `'g'` is fixed only in
roughly `[1e-4, 1e6]`, turning scientific at `1e7` on the large side, far earlier than JS's
`1e21`. All values are pinned below.

## Go: strconv

```go
strconv.FormatFloat(0.1, 'g', -1, 64)          // "0.1"          (shortest, matches JS digits)
strconv.FormatFloat(1e-5, 'g', -1, 64)         // "1e-05"        (sci already; JS still fixed)
strconv.FormatFloat(1e-4, 'g', -1, 64)         // "0.0001"       (fixed)
strconv.FormatFloat(1e7, 'g', -1, 64)          // "1e+07"        (sci already; JS still fixed to 1e20)
strconv.FormatFloat(1e6, 'g', -1, 64)          // "1000000"      (fixed)
strconv.FormatFloat(2.0, 'g', -1, 64)          // "2"            (matches JS "2")
strconv.FormatFloat(math.Copysign(0, -1), 'g', -1, 64) // "-0"   (JS prints "0")
strconv.FormatFloat(math.NaN(), 'g', -1, 64)   // "NaN"
strconv.FormatFloat(math.Inf(1), 'g', -1, 64)  // "+Inf"         (JS prints "Infinity")
```

Go's `'g'` switches to scientific at a much *smaller* magnitude than JS on both ends: fixed
only in roughly `[1e-4, 1e6]` (it goes sci by `1e-5` small and `1e7` large, where JS stays
fixed to `1e-6` and `1e20`). Its exponent is zero-padded to two digits (`1e-05`, `1e+07`).
`'g'` also prints non-finite values as `NaN`/`+Inf`/`-Inf` -- not the JS words -- and keeps
negative zero's sign. Round-trip is exact: `ParseFloat(FormatFloat(v, 'g', -1, 64), 64)`
recovers `v`'s bits for every finite value (verified across the value set, including
subnormals and `MaxFloat64`).

ParseFloat is *more* permissive than Number(): it accepts underscores (`"1_000"` -> `1000`),
hex floats (`"0x1p2"` -> `4`), and `NaN`/`Inf`/`Infinity`/`inf` case-insensitively. It
rejects hex *integer* literals (`"0x10"` -> error) and whitespace. Overflow errors rather
than returning `Inf` (`"1e999"` -> `value out of range`); underflow silently returns `0`.
Negative zero survives: `ParseFloat("-0")` keeps the sign bit.

To match JS byte-for-byte the lane will need to take the shortest digits from `'e', -1`
(which gives exact decimal digits plus exponent) and re-lay them out with the ECMA-262 rule,
rather than using `'g'` directly. This is the same re-layout the native and jq lanes already
built.

## Python: repr / float

```python
repr(0.1)                    # '0.1'          (shortest, matches JS digits)
repr(1e-5)                   # '1e-05'        (sci; JS still fixed at 1e-5)
repr(1e-4)                   # '0.0001'       (fixed)
repr(1e15)                   # '1000000000000000.0'  (fixed)
repr(1e16)                   # '1e+16'        (sci; JS still fixed at 1e16)
repr(2.0)                    # '2.0'          (JS prints "2")
repr(-0.0)                   # '-0.0'         (JS prints "0")
repr(float('inf'))           # 'inf'          (JS prints "Infinity")
repr(float('nan'))           # 'nan'          (JS prints "NaN")
```

`repr` switches to scientific on *both* ends earlier than JS: small side by `1e-5` (JS holds
fixed to `1e-6`), large side by `1e16` (JS holds fixed to `1e20`). Its exponent is
zero-padded (`1e-05`, `1e+16`). It keeps the `.0` on whole values (`2.0`) and keeps negative
zero's sign. Non-finite values come out lowercase (`inf`, `nan`). Round-trip is exact:
`float(repr(v))` recovers `v`'s bits (verified across the value set).

`float()` (the parse side) is the *most* permissive of the three: it accepts whitespace,
underscores (`"1_000"` -> `1000.0`), a leading `+`, `1.` / `.5` forms, and `NaN`/`Inf`/
`Infinity`/`nan`/`inf`/`infinity` case-insensitively. It rejects hex and binary literals
(`"0x10"`, `"0b101"`, `"0x1.8p1"` all raise) -- but `float.fromhex` *does* accept hex floats
(`float.fromhex("0x1.8p1")` -> `3.0`). Overflow gives `inf`, underflow gives `0.0`.

The Python lane has a triple divergence to close: the `.0`-on-whole-values, the earlier
scientific switch on both ends, and the lowercase non-finite words. Like Go, the clean path
is shortest digits from `repr` re-laid out per ECMA-262, not `repr` used directly.

## Rust: Display / Debug / parse

Rust is the odd one out: its `Display` (which `src/float.rs::lit` uses, and which is the
literal-spelling path every backend shares) **never** uses scientific notation.

```rust
format!("{}", 1e16f64)     // "10000000000000000"     (JS: "10000000000000000", same here)
format!("{}", 1e21f64)     // "1000000000000000000000" (JS: "1e+21" -- divergence)
format!("{}", 1e300f64)    // 309-digit fixed string   (JS: "1e+300")
format!("{}", 5e-324f64)   // long fixed string        (JS: "5e-324")
format!("{}", 2.0f64)      // "2"                      (matches JS)
format!("{}", -0.0f64)     // "-0"                     (JS: "0")
format!("{:?}", 1e16f64)   // "1e16"      (Debug: sci, no "+", no zero-pad)
format!("{:?}", 0.0001f64) // "0.0001"    (Debug stays fixed at 1e-4)
format!("{:?}", 1e-5f64)   // "1e-5"      (Debug sci at 1e-5)
```

Rust's `Display` prints the shortest round-trip digits (so round-trip via `parse` is exact,
verified), but it holds fixed notation across the entire representable range -- `f64::MAX`
comes out as a 309-digit fixed string, `5e-324` as a huge fixed string. That matches JS only
in the middle band where JS is also fixed; outside it, JS goes scientific and Rust does not.

Rust's `Debug` (`{:?}`) is the closest native primitive to what the reference needs: it uses
scientific notation for large and small magnitudes, and its exponent is unadorned (`1e16`,
`1e-5`, no `+` sign, no zero-pad). It diverges from JS only in the missing `+` sign and the
exact switch points (`1e-5` vs JS `1e-7` on the small side). So the Rust lane is
`Debug`-plus-a-sign-repair plus a re-layout, again the same ECMA-262 rule.

`str::parse::<f64>()` is the *least* permissive parser: it rejects whitespace, underscores
(`"1_000"` errors), and hex (both `"0x1p2"` and `"0x10"` error). It accepts
`NaN`/`Infinity`/`inf`/`infinity` case-insensitively, overflow to `inf` without error
(`"1e999"` -> `inf`), and it preserves negative zero (`"-0"` -> `-0.0`).

## The `lit` int-absorption hazard

`src/float.rs::lit` spells a finite Float literal as `n.to_string()` -- Rust's `Display`.
Two consequences the build lanes must not rediscover:

- `float::lit(2.0)` yields `"2"` (Rust `Display` drops the `.0`). JS accepts `2` as a Number
  and Python's `float("2")` parses it as a float, so for **formatting output** this is
  harmless. But a Python-lane emitter that writes the literal `2` into Python source gets an
  *int*, not a float -- the type and the runtime value disagree. Same shape the python lane
  was probing against.
- `float::lit` for a large/small magnitude emits the full fixed string (`f64::MAX` ->
  309 digits) because that is Rust `Display`. That is a valid literal on every backend but is
  enormous; it is not what JS prints, and a backend that compares emitted literal text
  against the JS formatter's output will think it is wrong.

Neither is a defect to fix in `float.rs` now (the task must not touch `src/`). They are
spelling hazards the three build lanes should route around: emit from the ECMA-262 digits
the backend actually needs, not from `lit`'s raw `Display` string, where the two disagree.

## What each lane does

All three lanes need the same shape: take shortest-round-trip decimal digits and re-lay them
with the ECMA-262 fixed/scientific rule and the JS word spellings. The per-backend entry
point differs:

- **Go**: shortest digits via `strconv.FormatFloat(v, 'e', -1, 64)` (exact digits + exponent,
  ignoring its notation), re-laid out; spell non-finite as `NaN`/`Infinity`/`-Infinity`;
  print `-0` as `0`.
- **Python**: shortest digits via `repr` (its digits match JS), re-laid out; strip the `.0`
  on whole values; spell `nan`/`inf` as `NaN`/`Infinity`; print `-0.0` as `0`.
- **Rust**: `Debug`-style digits (`{:e}` gives exact digits + exponent, or `{:?}`),
  re-laid out with a `+` sign repair; print `-0` as `0`.

The re-layout is already written twice in this repo (native: runtime/toylang.c
`tl_float_to_str`; jq: src/emit_jq.rs `FLOAT_PRINT_HELPER`). The Go/Python/Rust lanes are
not implementing a new algorithm; they are porting the ECMA-262 layout to a third, fourth,
and fifth host and confirming it against the JS reference across the notation boundaries and
the value set above.
