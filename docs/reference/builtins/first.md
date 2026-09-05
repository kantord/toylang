# first

`first(v)`, of type `Vec<T> -> Opt<T>`: the first entry of `v`. In the search model's
vocabulary it is the cut ([draft.md#query-is-search](../../../draft.md#query-is-search)): it
commits to the first answer and abandons the rest, so it is what makes a search stop at the
first hit rather than explore every branch.
```toylang
first([3, 1, 4])!
```

```output
3
```

The result is an `Opt` because an empty `Vec` has no first entry, and the absent `Opt` prints
as `null` -- the same answer indexing gives to absence:

```toylang
fn nothing() -> Vec<Int> = []

first(nothing())
```

```output
null
```

The first entry is a value like any other, records included:

```toylang
first([{n: 1}, {n: 2}])!
```

```output
{"n":1}
```

Any element type is accepted, so the only refusal is a scalar argument:

```toylang
first(1)
```

```error
`first` needs a Vec, found Int (at byte 6)
```