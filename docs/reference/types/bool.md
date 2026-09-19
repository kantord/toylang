# Bool

The type of a comparison, and what a condition must be. Its two values are `true` and `false`,
and they are constructors rather than keywords: the [prelude](../prelude/index.md) declares

```
pub enum Bool { True, False }
```

and, as for any [enum](enum.md), the lowercased variant names build the values. `Bool` itself
stays the built-in type a comparison yields, so the declaration adds nothing a program could
confuse with it: a written `Bool` is the same type whether the value came from `1 == 1` or from
the constructor, `Bool.true` is `true` spelled through its enum, and a program cannot declare a
second `Bool` any more than a second `Int`.

```toylang
{lit: true, cmp: 1 == 1, qualified: Bool.false}
```

```output
{"lit":true,"cmp":true,"qualified":false}
```

[`and`, `or`, and `not`](../operators/boolean.md) stay operators, with a literal as an ordinary
operand, and a Bool is consumed where it always was: by `select`'s predicate, by a
[match](../operators/match.md) guard, or by being the result. What the declaration adds is the
match over the value itself, closed-world over `True` and `False` like any enum match:

```case
bool_match
```

```toylang
true | True -> "yes"
```

```error
a match over `Bool` must cover every variant or end in a default; missing `False` (at byte 7)
```

As output and as input it is JSON's `true`/`false`, unchanged by the declaration; every backend
keeps its own boolean for the value rather than a tagged variant, so a Bool inside a record or a
Vec reads and prints in place.

```case
bool_input_round_trip
```
