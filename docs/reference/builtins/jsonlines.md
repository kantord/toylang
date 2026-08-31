# jsonlines

`jsonlines(v)`: prints each entry as JSON on its own line, instead of wrapping the whole
thing in `[...]`. Named for the format (jsonlines.org, also called NDJSON). It takes a
`Vec<T>` or a `Stream<T>`, and it is a sink: legal only as the program's outermost
expression, writing as it goes, with no result type, since nothing remains that could
observe one.

```toylang
jsonlines(["ada", "bo", "cy"])
```

```output
"ada"
"bo"
"cy"
```

Each entry is rendered as JSON, which is why the strings above are quoted where a top-level
`Str` result would print raw.

Over a stream, `jsonlines` is what makes a program incremental end to end: a
read-one, transform-one, write-one loop that prints each line before the next is read,
rather than holding stdin in memory. The corpus case pins the value; `tests/streaming.rs` is
what proves output arrives before stdin closes.

```case
jsonlines_of_inputs
```

Because a sink has no result, `jsonlines` cannot appear inside a larger expression:

```toylang
length(jsonlines([1, 2]))
```

```error
a sink is not a value, so it is legal only as the program's outermost expression or a Sink-returning function's body (at byte 7)
```
