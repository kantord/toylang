# all

`all(v)`, of type `Vec<Bool> -> Bool`: whether every entry of `v` is true. In the search
model's vocabulary it is the universal cut ([draft.md#query-is-search](../../../draft.md#query-is-search)): the
search stops at the first false answer, so an empty `Vec` -- which has no false entry at all --
is vacuously true.

```toylang
all([1 == 1, 2 == 2])
```

```output
true
```

One false entry is enough to fail the whole cut:

```toylang
all([1 == 1, 1 == 2])
```

```output
false
```

An empty `Vec` is vacuously true -- there is no false entry to refute it:

```toylang
fn no_misses() -> Vec<Bool> = []

all(no_misses())
```

```output
true
```

Defined only for a `Vec` of `Bool`: no other element type has a truth value to cut on.

```toylang
all([1, 2])
```

```error
`all` needs a Vec of Bool, found Vec<Int> (at byte 4)
```