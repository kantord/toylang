# BigInt in toylang: how comparable languages expose arbitrary precision

Tracking row [bigint-tracking (gh:112)](https://github.com/kantord/toylang/issues/112),
deliberately unscheduled. This is the survey the row is waiting on, written so a future
grilling round has the four models side by side and the decision surface named. No compiler
change is proposed here; [euler-ergonomics](euler-ergonomics.md#big-integers-re-tracked)
records why it stays a tracking row: a third integer type has real minimalism costs, and the
`Int`/`Int64` bridge discipline already shows the price of two.

The decision is much more constrained than the four languages make it look, because toylang
already answered the load-bearing questions once for `Int64`
([ADR 0010](../docs/adr/0010-int64-is-a-second-integer-type.md)). Most of the BigInt surface is
the same three choices applied to a type that cannot overflow. The survey below exists to
check those choices against languages that actually shipped arbitrary precision, and to make
visible where the `Int64` precedent does not transfer.

## The four models

What varies across languages is one axis: how the arbitrary-precision value relates to the
normal fixed-width int. Three answers exist, and the four languages split among them.

### Python: one int, no fixed width to interact with

There is a single `int` type and it is arbitrary precision; a fixed-width int exists only if
you import one from elsewhere (e.g. numpy's `int64`). `2**100` is just a value; nothing
overflows, so there is no promotion to design and no conversion surface. The cost is that the
*only* integer is the slow one, and the language's performance story has to live with it.

```python
>>> 2**100
1267650600228229401496703205376
>>> type(2**100) is int
True
```

Internally CPython stores `int` in base-2^30 limbs, so a small value is a small vector and
most arithmetic is fast in practice; the penalty is the object header and the lack of a
narrow hardware fast path, not the digit count itself.

### Rust with num-bigint: a separate type, explicit conversion, operator overloads

`BigInt`/`BigUint` from the `num-bigint` crate are distinct types from `i64`/`u64`. There is
no implicit promotion: `1_i64 + BigInt::from(2)` is a type error. Conversion is by name, both
directions, and the narrowing one is fallible:

```rust
use num_bigint::BigInt;
let wide = BigInt::from(1_i64) + BigInt::from(2);   // operator overload
let back: Option<i64> = wide.to_i64();              // None if it doesn't fit
```

This is the model toylang's `Int64` already is, minus the wrap: a real second type, a named
bridge, no silent widening. Arithmetic overloads `+`/`-`/`*`/`/` through the `std::ops`
traits, so the ergonomics are the same operators as the fixed-width int once you are inside
the type; the cost is that every crossing is a visible `BigInt::from`/`.to_i64()` call and a
lose-the-value `Option`. Performance-wise the type is whatever the crate is -- off by
default but pluggable (e.g. a `gmp` backend), which is the payoff of keeping it out of
the language core.

### Go with math/big: a separate type, no operators at all

`big.Int` is a struct, not an operator type: Go has no operator overloading, so all
arithmetic is method calls with explicit receiver semantics.

```go
x := big.NewInt(1)
x.Add(x, big.NewInt(2))        // x = x + 2
i := x.Int64()                 // lossy; also SetInt64, Text, etc.
```

The conversion surface is fully explicit and every operation names itself. Ergonomically it
is the heaviest of the four -- there is no `+` to reach for -- and it is the only model that
does not overload arithmetic, because the host forbids it. The mutating receiver
(`z.Add(x, y)`) is a consequence of avoiding allocation, and it is the model's one footgun:
operations mutate their receiver, which surprises anyone coming from a value language.

### JavaScript: a separate type, a literal suffix, refusal to mix

`BigInt` is a distinct primitive with its own literal suffix `n` (`123n`). It is the only
language here with a *spelling* for the type, and it is also the strictest about mixing:
`1n + 1` throws a `TypeError` rather than coercing, so the guard is at runtime, not the type
checker.

```js
let wide = 123n + 1n;
Number(wide);       // explicit, lossy
```

The suffix exists because JavaScript's normal int is a double and there was no other way to
write a value too big for one. The refusal to mix with `Number` is the same call toylang's
`Int64` makes -- never let the two meet silently -- but enforced at runtime because JS has no
compile-time type system to catch it in. The performance cost is real: `BigInt` arithmetic
is an order of magnitude off the engine's Smi fast path, which is exactly what toylang's
32-bit wrap exists to stay on (draft.md's "why 32 bits" argument at the value level, not
the type level).

## What transfers, and what does not

Three things in the survey confirm rather than complicate the `Int64` precedent, and one
breaks it.

- **A second type with a named bridge is the mainstream choice.** Python is the only one
  that absorbs bigints into the single int, and it is the only language here without a
  fixed-width fast path to preserve. toylang has one (the 32-bit wrap is what lets a
  reduction vectorise, [ADR 0006](../docs/adr/0006-int-is-32-bits-and-wraps.md)), so Python's
  model is the one that costs the thing the language is for.
- **No implicit widening, ever.** Rust, Go, and JS all refuse silent mixing. toylang's `Int64`
  already does, and the error already names the bridge. BigInt inherits this for free.
- **The narrowing direction is fallible and explicit.** Rust returns an `Option`, Go's
  `.Int64()` is silently lossy, JS's `Number()` is silently lossy. This is the one place the
  four genuinely differ, and it is a decision toylang has not had to make: `Int64 -> Int` does
  not exist at all, so there is no precedent for what `BigInt -> Int` should do when it does
  not fit.
- **The overload question is the one toylang has not answered.** toylang's `+` already
  dispatches on operand type for `Int` and `Str`; adding `BigInt` makes it a three-way
  dispatch, and nothing in the language settles whether that is welcome or a symptom. Go's
  answer (no operators) is available as a fallback that sidesteps the whole question, at the
  ergonomics cost the survey shows.

## Options

All three assume the `Int64` shape unless stated otherwise: literals resolve by position, the
bridge is named, and the type prints as a JSON number.

### Option A: a third integer type following ADR 0010

The direct extension. `BigInt` is a real type; a literal that fits neither `Int` nor `Int64`
resolves where a `BigInt` is expected (the same position rule `Int64` uses, applied twice);
`big(x)` is the one bridge in, exact because every `Int` and `Int64` fits. Once inside the
type, arithmetic is the ordinary operators, and a bare literal on the other side of one
resolves to `BigInt` by position:

```toylang
fn factorial(n: BigInt) -> BigInt =
    n == 0 -> 1 or
    n * factorial(n - 1)

factorial(50)
```

```output
30414093201713378043612608166064768844377641568960512000000000000
```

The bridge in is `big(x)`, converting a value carried as `Int` or `Int64` (its exact input
surface is open; the `i64` precedent takes only `Int`):

```toylang
fn digits(n: Int64) -> BigInt = big(n)

digits(600851475143)
```

```output
600851475143
```

The narrowing direction is left unbuilt until the fallible-conversion question is settled
(see open questions). The cost of the option is that `Int` and `Int64` arithmetic both have
to be widened by name to reach it, and `+` becomes a three-way dispatch.

This is the only option with a concrete precedent to copy, which is its main virtue: the
shape is already written down and already grilling-tested for `Int64`.

### Option B: make `Int` itself arbitrary precision

The Python model: no third type, the existing `Int` grows, and `Int64` (which exists to
*escape* `Int`'s ceiling) becomes redundant. A wide literal is just an `Int`.

```toylang
fn big() -> Int = 600851475143

big()
```

```output
600851475143
```

This is the smallest surface -- it deletes a type rather than adding one -- but it reverses
[ADR 0006](../docs/adr/0006-int-is-32-bits-and-wraps.md)'s wrap, and the wrap is load-bearing: it is what lets `+` vectorise without a
branch, and [ADR 0006](../docs/adr/0006-int-is-32-bits-and-wraps.md) argues that branch is
exactly what the language should not pay. It also strands `Int64` as a pointless duplicate,
and it makes `Int` slower on the backends where the 32-bit wrap maps to hardware. This is the
option that trades away the performance argument the other three preserve.

### Option C: BigInt as a combinator type, no operator overload

The Go model applied to toylang's style: `BigInt` exists, but `+` stays two-way and BigInt
arithmetic goes through named combinators or the function bridge.

```toylang
fn big_add(a: BigInt, b: BigInt) -> BigInt = ...
```

The virtue is that it dodges the three-way `+` dispatch entirely, and it dodges it honestly --
it never pretends BigInt arithmetic is free of a size decision. The cost is the survey's Go
result: every operation is a name, the ergonomics fall off a cliff, and a language whose
whole pitch is `+`-shaped math now asks the user to reach for `big_add` for the exact cases
where numbers are large. It also has no precedent in toylang -- there is no operator in the
language today that refuses to overload a type it could, so this option invents a new kind of
surface rather than extending an existing one.

## Open questions for a grilling round

- **Does `+` overload silently to a third type, or require a combinator?** Option A vs
  Option C. The `Int64` precedent says a new integer type joins `+`; nothing has tested
  whether three-way dispatch is still the same operator or has become a different one.
- **Is promotion automatic on overflow?** When an `Int` (or `Int64`) arithmetic result
  overflows, does it widen to `BigInt`, or keep wrapping? Every surveyed language keeps its
  fixed width and does not promote; toylang's vectorisation argument also says wrap. But
  automatic promotion is the one genuinely attractive-sounding alternative, and it needs the
  rejection written down rather than assumed.
- **What does `BigInt -> Int` do when it does not fit?** toylang has never had a fallible
  narrowing. Rust returns `None`, Go and JS silently lose the value. The choice (an `Opt`,
  an error, a silent wrap) is the one decision the `Int64` precedent does not cover.
- **Does `input` refuse `BigInt` like `Int64`?** The `Int64` read side is refused because the
  wire codec is not designed; BigInt would be strictly worse (JavaScript `BigInt` has no JSON
  representation at all). Likely the same refusal, but the precedent is for the *type* being
  barred, and BigInt is the case where reading it back is not merely undesigned but
  host-impossible on one backend.
- **What does jq do?** jq's only number is an IEEE double, and `Int64` already has a
  documented precision boundary at 2^53 because jq cannot follow past it. BigInt is that
  boundary made total: jq cannot represent arbitrary precision at all. The question is whether
  BigInt joins the corpus as another inside-the-envelope type (pointless for the values BigInt
  exists for) or whether jq becomes the first backend that refuses a type outright, which
  breaks the "every case runs on every backend" invariant.

## Notes on the survey's limits

The four-language facts above are stated from memory of well-established, stable behavior
(Python 3's unified `int`, Rust `num-bigint`'s `BigInt::from`/`to_i64`, Go `math/big`'s
method-only API, JS `BigInt`'s `n` suffix and refusal to mix with `Number`). None were
re-run against a live toolchain for this draft. The API signatures named are the long-stable
public ones for each; if a grilling round wants to rely on a specific version's behavior, the
snippets should be re-run the way
[float-format-research](float-format-research.md) does, since toolchains drift.
