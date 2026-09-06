# any

`any(v)`, of type `Vec<Bool> -> Bool`: whether any entry of `v` is true. In the search
model's vocabulary it is the existential cut ([draft.md#query-is-search](../../../draft.md#query-is-search)): the
search stops at the first true answer, so an empty `Vec` -- which has no true entry at all --
is false.

```toylang
any([1 == 1, 1 == 2])
```

```output
true
```

No true entry means no cut to make:

```toylang
any([1 == 2, 2 == 3])
```

```output
false
```

An empty `Vec` is false -- there is no true entry to find:

```toylang
fn no_hits() -> Vec<Bool> = []

any(no_hits())
```

```output
false
```

Defined only for a `Vec` of `Bool`: no other element type has a truth value to cut on.

```toylang
any([1, 2])
```

```error
`any` needs a Vec of Bool, found Vec<Int> (at byte 4)
```