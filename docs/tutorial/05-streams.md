# Streams and stdin

Programs so far carried their data in the source. Real ones read stdin, and there are three
ways in, at most one per program:

- `input`: one JSON value, read whole.
- `inputs`: a stream of JSON values, one per line.
- `lines`: a stream of raw text lines.

## One value: input

`input` is typed by where it is used, so hand it to a function whose signature says what
stdin must be:

```toylang
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users | select(.age >= 18) | map(.name)

adults input
```

```input
{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": 9}]}
```

```output
["ada"]
```

The value is validated against the declared type before the program runs -- a wrong shape
is refused, not coerced.

## Many values: inputs and lines

When input arrives as records -- log lines, exported rows -- it may not fit in memory, and
may not even end. `inputs` types stdin as a `Stream`: entries flow through the pipeline one
at a time, and the type system keeps it that way. A `Stream<T>` parameter accepts it, and
the same `select`/`map`/projection spellings work on it:

```toylang
fn adults(users: Stream<{name: Str, age: Int}>) -> Stream<{name: Str}> =
    users | select(.age >= 18) | map {name: .name}

jsonlines adults inputs
```

```input
{"name": "ada", "age": 36}
{"name": "bo", "age": 9}
{"name": "cy", "age": 21}
```

```output
{"name":"ada"}
{"name":"cy"}
```

`jsonlines(...)` at the end is a sink: it prints each entry as JSON on its own line, as it
arrives. This whole program compiles to a read-one, transform-one, write-one loop -- output
starts before stdin closes.

`lines` is the same shape for text that is not JSON; its entries are the raw lines, as
`Str`.

## Leaving the stream

A stream is not a value: it cannot be printed, stored, or indexed. `collect` is the one
exit, turning `Stream<T>` into an ordinary `Vec<T>` by reading everything:

```toylang
fn count(xs: Vec<Int>) -> Int = extent(xs)

count collect inputs
```

```input
1
2
3
```

```output
3
```

Prefer the sink when the program can print as it goes; `collect` when it genuinely needs
everything at once, such as to count. The [streams guide](../guides/streams.md) covers
choosing, and the rules the checker holds you to.

That is the core of the language. From here: the [guides](../guides/enums.md) for
feature-sized tasks, the reference for every builtin, type, and operator, and Examples for
every corpus program with the code all seven backends compile it to.
