# The conditional

`A if C else B`, spelled Python's way, and an expression: it has a value, so it appears
anywhere a value does, and both branches must have the same type. The condition is a
[Bool](../types/bool.md).

```toylang
"big" if 10 > 5 else "small"
```

```output
big
```

Chaining reads left to right, each `if` guarding the value before it, which lays a
cascade out flat:

```toylang
str(1) if 1 == 2 else str(2) if 1 == 1 else str(3)
```

```output
2
```

The cascade is how anything case-shaped over non-enum data is written -- FizzBuzz is the
canonical layout:

```case
fizzbuzz
```

There is no statement `if`: a program is one expression, so the conditional is the only
branching form outside a [match](match.md), and it always has an `else`.
