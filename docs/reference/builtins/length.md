# length

`length(v)`, of type `Vec<T> -> Int`: how many entries the outermost dimension has. A `Vec`
already tracks it at runtime, so reading it out costs nothing; there is no fold hiding behind
the name.

```toylang
length([10, 20, 30])
```

```output
3
```

Only the outermost dimension is counted. The length of a `Vec<Vec<Int>>` is the number of
inner `Vec`s, whatever their own lengths are:

```toylang
length([[1, 2, 3], [4]])
```

```output
2
```

`length` needs a `Vec`, so a stream must go through [`collect`](collect.md) first: a stream's
length is not knowable until it has been consumed.