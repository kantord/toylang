# stdin

`stdin` is the one source of external input, born `Stream<Str>`: the raw lines of stdin,
each handed over as the text it is, no parsing and no quoting. It is the input for data
that is lines of text rather than JSON.

```case
lines_cat
```

The line terminator is not part of the entry, an unterminated final line still arrives, and
a `\r` before the `\n` is preserved rather than stripped: the entries are the bytes between
terminators, not a platform's opinion of them.

Being a [Stream](../types/stream.md) source, `stdin` follows the stream rules: read at most
once per program (there is only ever one real stdin, so a second `stdin` is refused rather
than silently handed nothing), consumed exactly once, and not readable inside a `map` or
`select` body, which runs once per entry.

```case
jsonlines_of_lines
```

JSON input is reached through the [`parse`](../builtins/parse.md) builtin, not a separate
source. `parse(stdin)` reads the whole of stdin as one value of the checked type, the
spelling `input` retired into:

```case
adults
```

And `stdin | map(parse(.))` reads one JSON value per line into a `Stream<T>`, the spelling
`inputs` retired into -- the JSON Lines wire format, for input that arrives as records and
may not fit, or even end:

```case
jsonlines_of_inputs
```

Eager use has a visible spelling: `collect(stdin | map(parse(.)))` reads every remaining
value into a `Vec<T>` before the body runs.

```case
inputs_scalars
```

Before the program runs, each JSON value is validated against `T` -- a parse, not a
coercion. `{"age": "36"}` where `Int` was declared is an error, not a conversion; a number
that does not fit in 32 bits is refused; a missing declared field is refused; undeclared
fields are ignored. See [records as input](../types/record.md) and
[enums as input](../types/enum.md). Absence, `Char`, and `Int64` have no wire form to read,
so a `parse` whose result is one of those is refused.

`stdin` reads the same real stdin as [`dsv`](dsv.md), so a program uses at most one of the
two: any two together are refused, because they would read one resource two different ways.
