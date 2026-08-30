# Char

A single Unicode scalar value, never a surrogate half. The one way to get one is
[`chars`](../builtins/chars.md), which decodes a `Str` into a `Vec<Char>` by codepoint on
every backend, so a character outside the Basic Multilingual Plane is one `Char` even on a
target whose own strings need a surrogate pair to spell it:

```toylang
extent(chars("a😀b"))
```

```output
3
```

There is no `Char` literal. A program spells one by decoding a one-character `Str`:
`chars("0")[0]!` is the character `0`. `Char` supports the same comparisons `Int` does
(`==`, `!=`, `<`, `<=`, `>`, `>=`), and those plus
[`and`/`or`/`not`](../operators/boolean.md) are what a character class is built from.

A `Char` has no wire form: it is refused both as `input`'s type and as the program's own
printed result, since neither JSON nor either refusal has anything to decode or encode one
from or to. Everywhere else -- a function's parameters and return type, a record field, a
`Vec<Char>` -- it is an ordinary type.
