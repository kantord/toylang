# Result

`Result<T, E>`: a `T` on success or an `E` on failure. The prelude declares it as an
ordinary [enum](enum.md) with two type parameters -- `enum Result<T, E> { ok(T), err(E) }`
-- so it carries no machinery beyond what any generic enum gets. Unlike [`Opt`](opt.md),
`Result` is not special-cased: both variants are payload variants, and serialization keeps
the single-key wrapper rather than collapsing one side to `null`.

```toylang
fn safe_div(pair: {a: Int, b: Int}) -> Result<Int, Str> =
    ok(pair.a / pair.b) if pair.b != 0 else err("division by zero")

{good: safe_div({a: 10, b: 2}), bad: safe_div({a: 10, b: 0})}
```

```output
{"good":{"ok":5},"bad":{"err":"division by zero"}}
```

Consumption is the same [match](../operators/match.md) any enum uses, closed over `ok` and
`err`:

```case
result_match_both_arms
```

There is no builtin that produces a `Result` and no `!` for it -- `!` stays Opt's, peeling
tagged absence, which is a different question from which of two payloads a value holds.
A program builds and reads `Result` values entirely by hand, the same as any other
two-variant generic enum, until a decision gives specific operations (`safe_div`, an
unwrap-for-Result, or similar) their own producers.
