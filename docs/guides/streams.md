# Processing unbounded input

The task: stdin is a feed -- JSON records or text lines, too many to hold, possibly
endless -- and the program should emit results as it reads, in constant memory.

## Pick the source

`inputs` when each line is a JSON value, typed by the `Stream<T>` parameter that consumes
it; [`lines`](../reference/sources/lines.md) when the lines are plain text. (When stdin is
one document rather than a feed, this is not a streaming problem: `input` reads it whole.)

## Write the pipeline as if it were a Vec

`select`, `map`, and projection take a `Stream` subject and return a `Stream`, so the
pipeline reads the same as chapter 3's:

```toylang
fn shout(names: Stream<Str>) -> Stream<Str> =
    names | select(. != "bo") | map(. + "!")

jsonlines(shout(lines))
```

```input
ada
bo
cy
```

```output
"ada!"
"cy!"
```

End at the `jsonlines` sink and the compiler fuses the whole chain into a read-one,
transform-one, write-one loop: output starts before stdin ends, and nothing accumulates.
The corpus pins these values; `tests/streaming.rs` is what proves output arrives while
stdin is still open.

## Collect only what must be whole

Counting, indexing, `extent`, putting results in a record -- anything that needs the whole
extent -- goes through `collect`, which waits for everything:

```case
inputs_scalars
```

The honest shape is often both: stream through the transformation, collect the small
result, not the input.

## The rules the checker holds you to

Each of these is a compile-time refusal, not a runtime surprise:

- One source per program, used once: two reads of the same stdin cannot both be right.
- No source inside a `map` or `select` body -- it runs once per entry, and stdin cannot be
  read once per entry.
- A stream never sits inside a record, a `Vec`, or another `Stream`, and cannot be printed;
  it is not a value.
- A function returns a `Stream` only if one came in through its parameter
  ([Stream](../reference/types/stream.md) shows the refusal): streams are born at sources,
  so the pipeline stays one chain the compiler can fuse.

When a rule bites, the usual fix is to move the source to the program body and pass the
stream in as a parameter -- which is also what keeps the function reusable against any
stream, not just stdin.
