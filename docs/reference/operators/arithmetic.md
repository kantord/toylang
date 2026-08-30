# Arithmetic and +

`+ - * / %` on `Int` (and on [Int64](../types/int64.md), with the same rules at twice the
width -- the two never mix in one operator), with ordinary precedence (`*`, `/`, `%` bind
tighter) and parentheses to override:

```toylang
str(2 + 3 * 4) + "," + str((2 + 3) * 4)
```

```output
14,20
```

All of it wraps at 32 bits; see [Int](../types/int.md). Division truncates toward zero, and
the remainder takes the sign of the dividend:

```toylang
str(-7 / 2) + "," + str(-7 % 3) + "," + str(7 % -3)
```

```output
-3,-1,1
```

A zero divisor is the one way arithmetic can fail, and every backend refuses it at runtime
rather than producing a value:

```toylang
str(1 / 0)
```

```refuses
```

Unary minus negates, and the most negative `Int` is writable directly.

`+` is also `Str` concatenation, and `Vec` concatenation of two `Vec`s of the same element
type:

```toylang
[1, 2] + [3]
```

```output
[1,2,3]
```

That is the whole overload. It does not mix types, so `"n=" + 3` is refused rather than
coerced -- write `"n=" + str(3)`. Joining an unknown number of `Vec`s, such as one built by
`map`, is [`flatten`](../builtins/flatten.md) instead; the open question of what other
element-wise arithmetic over two `Vec`s should mean stays open.
