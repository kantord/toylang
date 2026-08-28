# concat

<!-- @comment Coordinator, resolved per your answer: concat stays as-is. Your call was that
renaming becomes unnecessary once proper function overloading and Rust-style traits land --
that direction now has its own decide row (overloading-and-traits-design), and this name is
one of its first customers. Read and settled. -->

`concat(vv)`, of type `Vec<Vec<T>> -> Vec<T>`: flattens one level of nesting, keeping order.

```toylang
concat([[1, 2], [3], [4, 5]])
```

```output
[1,2,3,4,5]
```

Exactly one level. Deeper nesting stays where it was:

```toylang
concat([[[1], [2]], [[3]]])
```

```output
[[1],[2],[3]]
```

Joining two `Vec`s is this same operation, spelled as a two-entry outer literal:

```toylang
concat([[1, 2], [3, 4]])
```

```output
[1,2,3,4]
```

Not to be confused with the `+` operator, which concatenates two `Str`s. `concat` is about
dimensions, `+` is about text.
