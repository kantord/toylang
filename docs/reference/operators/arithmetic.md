# Arithmetic and +

`+ - * / %` on `Int` (and on [Int64](../types/int64.md), with the same rules at twice the
width, and on [Float](../types/float.md), with IEEE rules instead -- no two of the three ever
mix in one operator), with ordinary precedence (`*`, `/`, `%` bind
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

A zero divisor is the one way integer arithmetic can fail, and every backend refuses it at
runtime rather than producing a value (a `Float` divisor of zero is the IEEE answer instead;
see [Float](../types/float.md)):

```toylang
str(1 / 0)
```

```refuses
```

Unary minus negates, and the most negative `Int` is writable directly. It stays a prefix operator rather than being folded into the lexer,
because `a -1` still has to mean `a - 1`.

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
`map`, is [`flatten`](../builtins/flatten.md) instead.

Every other arithmetic operator over two `Vec`s of the same numeric element type is
cartesian, jq's own default ([the multiplicity
question](../../../plans/questions.md#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit)):
every pair of elements is combined, and the result is laid out in jq's order, right operand
outermost, so the left side runs fastest:

```toylang
[2, 3] - [10, 20]
```

```output
[-8,-7,-18,-17]
```

A `Vec` on one side only is a type mismatch, not a broadcast: `[2, 3] * 10` is refused. What
a `Vec` meeting a scalar should mean is not decided.
