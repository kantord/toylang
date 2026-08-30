# sort

`sort(v)`, of type `Vec<T> -> Vec<T>`: `v`'s entries in ascending order, by the same total
order `<` already gives `T`. One value in and one value out -- the whole `Vec` has to be there
before the first output entry can be, so `sort` has no lawful `Stream` instance and takes only
a `Vec` ([Q20](../../../draft.md), kantord/toylang#86).

```toylang
sort([3, 1, 2])
```

```output
[1,2,3]
```

Restricted to the element types ordering already typechecks on -- `Int`, `Int64`,
[`Str`](../types/str.md), and [`Char`](../types/char.md) -- rather than reaching past what
every backend can order natively:

```toylang
sort(["banana", "apple", "cherry"])
```

```output
["apple","banana","cherry"]
```

```toylang
sort([{n: 1}])
```

```error
`sort` needs a Vec of Int, Int64, Str, or Char, found Vec<{n: Int}> (at byte 5)
```
