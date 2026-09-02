# range

`range(n)`, of type `Int -> Stream<Int>`: the integers from zero up to but not including `n`,
one at a time, as a stream. Zero-based, matching jq, Python, and the language's own indices.
`range` is one of the three stream sources (with `inputs` and `lines`), so a pipeline over it
fuses into a count-one/transform-one/write-one loop instead of materializing the whole `Vec`:

```toylang
jsonlines(range(5) | select(. >= 2) | map(. * 10))
```

```output
20
30
40
```

A zero or negative argument yields no elements, not an error:

```toylang
jsonlines(range(-3))
```

```output
```

As a stream it follows the same rules the other sources do: born at the source, dying at
`collect` or the `jsonlines` sink, never stored. Where a `Vec` is wanted, the eager spelling
makes the memory cost visible:

```toylang
collect(range(4))
```

```output
[0,1,2,3]
```

and everything that works on a `Vec` works on that:

```toylang
length(collect(range(-3)))
```

```output
0
```
