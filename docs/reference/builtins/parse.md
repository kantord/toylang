# parse

`parse(s)`, of type `Str -> T`: read the string `s` as one JSON value of the checked type
`T`. It is the same type-descriptor-driven parse every backend applies to stdin, available
to a string already in hand. The result type comes only from the position the call sits in,
the same way `input` always borrowed its type.

```toylang
parse("[1, 2, 3]")
```

```output
[1,2,3]
```

The two stdin spellings are the other face of `parse`. `parse(stdin)` reads the whole of
stdin as one value:

```case
adults
```

and `stdin | map(parse(.))` reads stdin one JSON value per line into a
[Stream](../types/stream.md):

```case
enum_inputs
```

The check is a parse, not a coercion: `{"age": "36"}` where `Int` was expected is an error,
and so is a number that does not fit in 32 bits. Absence, `Char`, and `Int64` have no wire
form to read, so a `parse` whose result is one of those is refused, exactly as `input` always
was.
