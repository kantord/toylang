# chars

`chars(s)`, of type `Str -> Vec<Char>`: every Unicode scalar value in `s`, in order. Decoded by
codepoint on every backend, not by byte and not by UTF-16 code unit, so a character outside the
Basic Multilingual Plane is one element here even on a target whose own strings need a
surrogate pair to spell it.

```toylang
extent(chars("abc"))
```

```output
3
```

A character outside the Basic Multilingual Plane is still one element, the difference between
this and counting bytes or UTF-16 units:

```toylang
extent(chars("a😀b"))
```

```output
3
```

There is no `Char` literal, so a program spells a boundary to compare against by decoding a
one-character `Str`: `chars("0")[0]!` is the character `0`. `Char` supports the same comparisons
`Int` does, which is what lets a character class be written as a range: two bounds joined with
[`and`](../operators/boolean.md), and its complement with `not`.

```case
char_range_and_complement
```

A `Char` has no wire form: it never comes from JSON, and it cannot be handed to the printer.
Both directions are refused at compile time:

```toylang
fn use_it(c: Char) -> Bool = c == c

use_it(input)
```

```error
`input` cannot be read as Char; Char has no wire form to read (at byte 44)
```
