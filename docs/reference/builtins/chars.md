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
`Int` does, which is what lets a character class be written as a range -- there is no `and`
either, so two bounds combine through the conditional, the same way any two-part test does:

```toylang
fn is_digit(c: Char) -> Bool =
    c <= chars("9")[0]! if c >= chars("0")[0]! else 1 == 0

extent(chars("a1b2c3") | select(is_digit(.)))
```

```output
3
```

Complement is ordinary `Bool` negation -- there are no `true`/`false` literals either, so it is
spelled against a comparison that is one, the way any boolean constant is:

```toylang
fn is_digit(c: Char) -> Bool =
    c <= chars("9")[0]! if c >= chars("0")[0]! else 1 == 0

fn is_not_digit(c: Char) -> Bool = is_digit(c) != (1 == 1)

extent(chars("a1b2c3") | select(is_not_digit(.)))
```

```output
3
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
