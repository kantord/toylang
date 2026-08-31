# max

`max(v)`, of type `Vec<Int> -> Opt<Int>` and `Vec<Int64> -> Opt<Int64>`: the greatest of `v`'s
entries (kantord/toylang#140). An empty `Vec` has no maximum, so the result is `Opt<T>` -- the
same answer indexing gives to absence, and more honest than Python's `max([])` exception or jq's
`add` returning null untyped.

```toylang
max([3, 1, 4, 1, 5])
```

```output
5
```

A maximum can be negative; the least negative is the greatest:

```toylang
max([-5, -1, -3])
```

```output
-1
```

An empty `Vec` yields the absent `Opt`, which prints as `null`:

```toylang
fn nothing() -> Vec<Int> = []

max(nothing())
```

```output
null
```

Defined only for the two integer element types. A `Vec` of records has no total order to
reduce to a maximum over, so it is refused:

```toylang
max([{n: 1}])
```

```error
`max` needs a Vec of Int or Int64, found Vec<{n: Int}> (at byte 4)
```
