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
enum Shape { point, circle{r: Int} }

circle({r: 1}) == circle({r: 1})
```

```output
true
```

Since [field order is not part of a record type](../types/record.md), two spellings of one
type are one value and compare equal. All of it is pinned on every backend:

```case
comparison_semantics
```

Equality stops at a `Vec`. `[1, 2] == [1, 2]` is refused, and so is
`{a: [1, 2]} == {a: [1, 2]}`, because what an operator applied to a dimension means --
compare the whole thing, or compare entry by entry and hand back a `Vec<Bool>` -- is a
question the language has not answered, and a record field is no better a place to answer it
by accident than the top level is.

Ordering also typechecks on `Str`, and it compares by Unicode codepoint on every backend --
including the JavaScript target, whose native `<` compares UTF-16 code units instead and so
disagrees with the other six on any pair straddling a surrogate pair; the emitted code steps by
codepoint there instead of using `<` directly.

One lexical trap: postfix `!` (unwrap) followed by `==` needs the space, because `!=` wins
the token. `v[0]!= 1` is a type error about `Opt<Int>`; the intended comparison is spelled
`v[0]! == 1`.
