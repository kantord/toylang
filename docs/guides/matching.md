# Choosing a branching form

The task: a program branches -- on a condition, on which variant of an enum arrived, or on
whether anything applies at all. All three go through match; which arm shape fits depends on
the data.

## Replacing an if/else chain

A guard chain (`cond -> body or cond -> body or ... or default`) is `if`/`else` in another
notation -- [the conditional reference](../reference/operators/conditional.md) lays FizzBuzz
out both ways. It holds up better once a cascade grows past two or three branches, because
each condition sits next to its own result instead of nesting another `else`:

```toylang
fn shipping(kg: Int) -> Int =
    kg | . <= 1 -> 5 or . <= 5 -> 12 or . <= 20 -> 25 or 40

{a: shipping(1), b: shipping(3), c: shipping(20), d: shipping(25)}
```

```output
{"a":5,"b":12,"c":25,"d":40}
```

## Matching enums

The [enums guide](enums.md) covers typing wire data and matching by variant end to end. One
thing worth knowing going in: a guard arm can sit in the same chain as variant arms, but it
never finishes the job. Coverage is checked on the pattern, not the runtime value, so even a
guard that always holds does not close the match:

```toylang
enum S { a, b }

S.a | a -> 1 or 1 == 1 -> 2
```

```error
a match over `S` must cover every variant or end in a default; missing `b` (at byte 23)
```

Name the remaining variant, or end the chain in `any()`:

```toylang
enum S { a, b }

S.a | a -> 1 or any() -> 2
```

```output
1
```

## Partial chains and their Opt results

A guard chain with no default is a legitimate way to say "some inputs don't classify" --
declining every arm produces `Opt`, not a refusal:

```toylang
fn discount(total: Int) -> Opt<Int> =
    total | . >= 100 -> total / 10

{a: discount(150), b: discount(50)}
```

```output
{"a":15,"b":null}
```

Consume the result the way any `Opt` is consumed: `!` if absence should become a runtime
error ([unwrap](../reference/operators/unwrap.md)), or fold a default into the chain itself
if it should not:

```toylang
fn discount(total: Int) -> Int =
    total | . >= 100 -> total / 10 or 0

{a: discount(150), b: discount(50)}
```

```output
{"a":15,"b":0}
```

The one thing a partial chain cannot do is return `Opt` from its own arms: a declined chain
and an arm that matched but found nothing would both print `null`, with no way to tell them
apart, so the checker refuses it:

```toylang
fn first_reading(entry: {valid: Bool, readings: Vec<Int>}) -> Opt<Int> =
    entry | .valid -> entry.readings[0]

first_reading({valid: 1 == 2, readings: [5]})
```

```error
this guard chain is partial, and its arms are already Opt<Int>: a declined chain and a matched-but-absent value would both print `null`; add a default arm (at byte 85)
```

Adding a default resolves it -- even one that can itself come back absent -- because the
chain is no longer partial; every input now hits an arm:

```toylang
fn first_reading(entry: {valid: Bool, readings: Vec<Int>}) -> Opt<Int> =
    entry | .valid -> entry.readings[0] or entry.readings[9]

{a: first_reading({valid: 1 == 2, readings: [5]}), b: first_reading({valid: 1 == 1, readings: [1]})}
```

```output
{"a":null,"b":1}
```

The same chain also runs directly per element inside `map`, guards and field reads over `.`
with no named function in between:

```case
map_guard_chain_over_vec_field
```

The [match reference](../reference/operators/match.md) has the full arm-shape rundown; the
[matching tutorial chapter](../tutorial/06-matching.md) is the narrative version.
