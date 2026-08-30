# Vec

`Vec<T>`: one dimension of entries, all of type `T`, addressed by position. The glossary
([CONTEXT.md](../../../CONTEXT.md)) calls the axis a dimension and its entry count the
extent; a `Vec<Vec<Int>>` has two dimensions, fixed by the type in order.

A `Vec` literal builds one from its entries:

```toylang
[1, 2, 3]
```

```output
[1,2,3]
```

The entries are where the element type comes from -- unless the position already says what
the `Vec` holds. A declared type flows into the expression it annotates, so under a return
annotation an empty `[]` takes its element type from the signature:

```toylang
fn nothing() -> Vec<Int> = []

nothing()
```

```output
[]
```

Where nothing expects anything, an empty literal still names no element type and is refused:

```toylang
[]
```

```error
cannot tell what `[]` contains (at byte 0)
```

What applies to a `Vec`: the index specs (`[0]`, `[-1]`, `[]` mid-chain -- see
[specs](../operators/specs.md)), [`extent`](../builtins/extent.md),
[`flatten`](../builtins/flatten.md), [`tail`](../builtins/tail.md),
[`sort`](../builtins/sort.md), [`reverse`](../builtins/reverse.md), the subject-fed
[`select`](../builtins/select.md) and [`map`](../builtins/map.md), and `+`, which joins two
`Vec`s of the same element type (see [arithmetic](../operators/arithmetic.md)).

Entries of a dimension need not have equal extents one dimension down; the glossary calls
such a value ragged, and collapsing an inner dimension of one yields `Opt` where entries are
missing rather than failing:

```case
index_ragged_inner
```
