# Matching

Chapter 4 used match to decode an enum: one arm per named variant, chained with `or`. A
match arm has a second shape -- a guard, a Bool test over any subject, not only an enum --
and the two compose in the same chain.

```toylang
fn grade(score: Int) -> Str =
    score | . >= 90 -> "A" or . >= 80 -> "B" or . >= 70 -> "C" or "F"

{a: grade(95), b: grade(82), c: grade(55)}
```

```output
{"a":"A","b":"B","c":"F"}
```

`.` stays the subject through a guard arm, in both the test and the body -- unlike a bare
payload arm, which rebinds it. The trailing `"F"` is a bare expression, the chain's default,
and only the last element can be one. Nothing checks that: an `or` following something that
is not a finished arm is read as Bool disjunction, so a bare expression in the middle becomes
a disjunct of whatever follows it, and is refused for not being a `Bool`:

```toylang
1 | . == 1 -> "one" or "other" or . == 2 -> "two"
```

```error
expected Bool, found Str (at byte 23)
```

## `or`, twice

That is the same `or` doing both jobs. A guard is still being read when its `or` arrives, so
the `or` joins clauses into one test; a body is finished, so the `or` after it ends the arm:

```toylang
fn size(n: Int) -> Str =
    n | . == 0 or . == 1 -> "tiny" or . < 10 -> "small" or "big"

{a: size(0), b: size(4), c: size(40)}
```

```output
{"a":"tiny","b":"small","c":"big"}
```

The one thing this costs: a Bool `or` written directly in an arm's body needs parens, since
the bare spelling there is the separator. `and` and `not` have no such wrinkle --
[the boolean operators reference](../reference/operators/boolean.md) covers all three.

## Guards read like if/else

Chapter 3's FizzBuzz is a cascade of conditions; as guard arms each condition sits next
to its own result, and the chain's final bare expression is the default:

```case
fizzbuzz_arms
```

## Hybrid totality

A variant match is closed-world: chapter 4 showed that leaving a variant out is refused,
naming it, unless the chain ends in `any()`. A guard is a runtime Bool the checker cannot see
through, so no guard -- however certain -- contributes to that coverage:

```toylang
enum S { a, b }

S.a | a -> 1 or 1 == 1 -> 2
```

```error
a match over `S` must cover every variant or end in a default; missing `b` (at byte 23)
```

`1 == 1` always matches at runtime, but the checker only counts named variants and `any()`.
Naming `b` or adding `any()` are the only two ways to close this chain.

## Partial guard chains and Opt

A pure guard chain -- no variant patterns, just guards -- may skip the default entirely.
Declining every arm is not a compile error: it is the same `Opt` an out-of-range index
already gives, printed as `null`:

```case
match_partial_guards
```

A declared `Opt<T>` return reaches the arms as `T`, not `Opt<T>` -- the chain's own
partiality is what supplies the `Opt`, so an arm that writes a bare `[]` resolves it against
the declared element type instead of needing it spelled out some other way:

```toylang
fn tags(n: Int) -> Opt<Vec<Int>> = n | . > 0 -> []

{a: tags(1), b: tags(-1)}
```

```output
{"a":[],"b":null}
```

When a partial chain's arms are themselves `Opt`-typed, the wrapping doubles: "no arm
matched" and "matched, and found nothing" are two different values in memory (`none` and
`some(none)`), even though both print `null` once serialization flattens the tags away.
The return type says so honestly:

```toylang
fn first_reading(entry: {valid: Bool, readings: Vec<Int>}) -> Opt<Opt<Int>> =
    entry | .valid -> entry.readings[9]

first_reading({valid: 1 == 2, readings: [5]})
```

```output
null
```

A default arm collapses the doubling, because the chain is no longer partial:

```toylang
fn first_reading(entry: {valid: Bool, readings: Vec<Int>}) -> Opt<Int> =
    entry | .valid -> entry.readings[0] or entry.readings[9]

{
    a: first_reading({valid: 1 == 2, readings: [5]}),
    b: first_reading({valid: 1 == 1, readings: [1]})
}
```

```output
{"a":null,"b":1}
```

That is the core of the language. From here: the [matching guide](../guides/matching.md) for
choosing a branching form and handling a partial chain's result, the other
[guides](../guides/enums.md) for feature-sized tasks, the reference for every builtin, type,
and operator, and Examples for every corpus program with the code all seven backends compile
it to.
