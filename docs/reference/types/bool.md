# Bool

The type of a comparison, and what a condition must be. There are no `true`/`false`
literals: a `Bool` is born from `==`, `!=`, `<`, `<=`, `>`, `>=`, and is consumed by the
[conditional](../operators/conditional.md), by `select`'s predicate, or by being the result.

```toylang
1 == 1
```

```output
true
```

As output and as input it is JSON's `true`/`false`. A program that needs a boolean constant
writes a comparison that is one, such as `1 == 1`; nothing shorter exists yet.
