# extent

`extent(v)`, of type `Vec<T> -> Int`: how many entries the outermost dimension has. Named for
the glossary term ([CONTEXT.md](../../../CONTEXT.md)) rather than `length`. A `Vec` already
tracks its extent at runtime, so reading it out costs nothing; there is no fold hiding behind
the name.

```toylang
extent([10, 20, 30])
```

```output
3
```

Only the outermost dimension is counted. The extent of a `Vec<Vec<Int>>` is the number of
inner `Vec`s, whatever their own extents are:

```toylang
extent([[1, 2, 3], [4]])
```

```output
2
```

`extent` needs a `Vec`, so a stream must go through [`collect`](collect.md) first: a stream's
extent is not knowable until it has been consumed.
