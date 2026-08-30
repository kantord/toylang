# reverse

`reverse(v)`, of type `Vec<T> -> Vec<T>`: `v`'s entries in the opposite order. Blocking for the
same reason [`sort`](sort.md) is -- one value in, one value out, no lawful `Stream` instance
([Q20](../../../draft.md), kantord/toylang#86) -- but needs no comparison, so it places no
restriction on `T`.

```toylang
reverse([1, 2, 3])
```

```output
[3,2,1]
```

```toylang
reverse([{n: 1}, {n: 2}])
```

```output
[{"n":2},{"n":1}]
```
