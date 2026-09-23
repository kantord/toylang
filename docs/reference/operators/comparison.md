# Comparisons

`==  !=  <  <=  >  >=`, each yielding [Bool](../types/bool.md). Both operands must have the
same type; nothing is coerced.

```toylang
[1, 2, 3] | select(. >= 2)
```

```output
[2,3]
```

Equality works on `Str` as well as `Int`:

```toylang
"ada" != "bo"
```

```output
true
```

`==` and `!=` reach inside a composite. Two records are equal when their fields are, two
enum values when they are the same variant carrying equal payloads, and neither asks where
the value came from:

```toylang
enum Shape { Point, Circle { r: Int } }


circle { r: 1 } == circle { r: 1 }
```

```output
true
```

Since [field order is not part of a record type](../types/record.md), two spellings of one
type are one value and compare equal. All of it is pinned on every backend:

```case
comparison_semantics
```

A comparison over two `Vec`s of the same element type is cartesian, the same rule as
[arithmetic](arithmetic.md): every pair is compared, right operand outermost, and the answer
is a `Vec<Bool>`:

```toylang
[1, 5] < [3, 4]
```

```output
[true,false,true,false]
```

That rule covers two bare `Vec`s and nothing deeper. `{a: [1, 2]} == {a: [1, 2]}` is refused,
and so is `[[1]] == [[1]]`, because what equality means for a `Vec` *inside* a value --
compare it as a whole, or reach in and hand back Bools -- is a question the language has not
answered, and a record field is no better a place to answer it by accident than a nested
`Vec` is.

`<`, `<=`, `>` and `>=` are not defined on a record or an enum, `Opt` included, and the
checker refuses them: `{a: 1} < {a: 2}` and `v[0] < v[1]` are type errors, the second until
each side is unwrapped. The same goes for a `Vec` of either, since the cartesian rule above
would order its elements one pair at a time. Before the refusal the seven backends answered
five different ways, from a compile error in the emitted Rust and Go to jq's own document
order, and no order on these types had been ruled the right one. `==` and `!=` on them are
unaffected.

What ordering does apply to is `Int`, `Int64`, `Float`, `Char`, `Bool` (`false` before
`true`), `Str`, and two bare `Vec`s of those elementwise. One case pins all of them:

```case
ordering_legal_types
```

Ordering on a `Str` compares by Unicode codepoint on every backend --
including the JavaScript target, whose native `<` compares UTF-16 code units instead and so
disagrees with the other six on any pair straddling a surrogate pair; the emitted code steps by
codepoint there instead of using `<` directly.

One lexical trap: postfix `!` (unwrap) followed by `==` needs the space, because `!=` wins
the token. `v[0]!= 1` is a type error about `Opt<Int>`; the intended comparison is spelled
`v[0]! == 1`.
