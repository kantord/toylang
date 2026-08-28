# inputs

`inputs` is stdin as a [Stream](../types/stream.md): one JSON value per line, the JSON
Lines wire format, typed `Stream<T>` where `T` comes from the position that consumes it.
Where [`input`](input.md) reads one document whole, `inputs` is for input that arrives as
records and may not fit, or even end.

The streaming shape end to end -- filter and reshape each record as it arrives, print as
you go:

```case
jsonlines_of_inputs
```

Eager use has a visible spelling: `collect(inputs)` reads every remaining value into a
`Vec<T>` before the body runs.

```case
inputs_scalars
```

Each line is validated against `T` the way `input` is validated against its type; a line
that misses it -- a string that names no variant of a declared enum, a number where a
record was declared -- is refused rather than coerced.

Blank lines are skipped. Being a stream, `inputs` obeys the stream rules: consumed exactly
once, and never alongside `input` or [`lines`](lines.md), which read the same stdin.
