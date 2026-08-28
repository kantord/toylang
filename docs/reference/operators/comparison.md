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

Ordering also typechecks on `Str`, but only its ASCII behavior is pinned by the corpus; the
backends' string representations are not guaranteed to agree beyond it, so ordering
non-ASCII text is not yet a promise the language makes.

One lexical trap: postfix `!` (unwrap) followed by `==` needs the space, because `!=` wins
the token. `v[0]!= 1` is a type error about `Opt<Int>`; the intended comparison is spelled
`v[0]! == 1`.
