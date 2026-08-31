# sum

`sum(v)`, of type `Vec<Int> -> Int` and `Vec<Int64> -> Int64`: the reduction of `+` over `v`'s
entries, at the element type's own width (kantord/toylang#140). An empty `Vec` sums to `0`.

```toylang
sum([1, 2, 3, 4])
```

```output
10
```

Each addition wraps the way the language's `+` does, so a sum that leaves the 32-bit width is
the same number a hand-written fold would produce:

```toylang
sum([2147483647, 1])
```

```output
-2147483648
```

The result keeps the element width, so a `Vec<Int64>` sums to an `Int64`:

```toylang
fn wide(x: Int) -> Int64 = i64(x)

sum([wide(3), wide(1), wide(2)])
```

```output
6
```

Defined only for the two integer element types. A `Vec<Str>` is refused rather than summed,
since there is no caller for it:

```toylang
sum(["a"])
```

```error
`sum` needs a Vec of Int or Int64, found Vec<Str> (at byte 4)
```
