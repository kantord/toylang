# Stream

`Stream<T>`: effect-layer multiplicity
([ADR 0001](../../adr/0001-stream-is-the-effect-layer-typed.md)). The type says an
expression yields its entries one at a time as evaluation proceeds, not that a stream object
exists as a value.

A stream is born only at a source -- [`inputs`](../sources/inputs.md),
[`lines`](../sources/lines.md), or [`range`](../builtins/range.md) -- and dies at
[`collect`](../builtins/collect.md) or at the [`jsonlines`](../builtins/jsonlines.md) sink. In
between, `select`, `map`, and projection accept a `Stream` subject and yield a `Stream` back,
so a whole pipeline can live in the effect layer:

```case
lines_stream_signature
```

The rules, each of which the checker enforces:

- A stream is consumed exactly once; using one twice would re-run the source twice.
- A stream never sits inside a record, a `Vec`, or another `Stream`.
- A stream cannot be printed. It is not a value; `collect` is what makes one.
- A function cannot conjure a stream: a signature may return `Stream` only if a `Stream`
  came in through its parameter, so the pipeline stays one chain from source to sink.

```toylang
fn conjure(n: Int) -> Stream<Int> = lines | map(n)

0
```

```error
`conjure` returns Stream<Int> without taking a stream; a stream is born only at a source (at byte 0)
```

What the chain buys is fusion: a pipeline of the right shape compiles to a read-one,
transform-one, write-one loop on every backend, printing each entry before the next is read.
`tests/streaming.rs` is what proves that; the corpus pins only the values.
