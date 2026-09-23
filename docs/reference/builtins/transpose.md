# transpose

`v | transpose`, of type `Vec<Vec<T>> -> Vec<Vec<T>>`: the rows and columns of the rectangular
`v` are swapped, so the entry at row `r`, column `c` of the input is the entry at row `c`,
column `r` of the result (gh:181). A `Vec` of rows only, never a stream -- the same blocking
`flatten` and `reverse` use.

The subject must be a rectangular `Vec<Vec<T>>`: every row has to have the same length. The
checker cannot see lengths, so a ragged input -- a `Vec` whose rows are not all the same length
-- is refused at runtime the same way every other hard failure is.

```toylang
transpose([[1, 2, 3], [4, 5, 6]])
```

```output
[[1,4],[2,5],[3,6]]
```

It runs on every backend, and a ragged input is refused on every one:

```toylang
transpose([[1, 2], [3]])
```

```refuses
```
