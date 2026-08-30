# tail

`tail(v)`, of type `Vec<T> -> Opt<Vec<T>>`: everything after the first entry. The result is
an `Opt` because the empty `Vec` has no tail, the same way an out-of-range index has no
entry; `!` is how a program insists the value is there.

```toylang
tail([1, 2, 3])!
```

```output
[2,3]
```

The tail of a one-entry `Vec` is present, and empty:

```toylang
length(tail([1])!)
```

```output
0
```

Unwrapping the tail of an empty `Vec` is the same mistake as unwrapping an absent index, and
every backend refuses it at runtime:

```toylang
tail(tail([1])!)!
```

```refuses
```
