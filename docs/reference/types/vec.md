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

The entries are where the element type comes from, which is why a bare `[]` is refused: an
empty literal names no element type, and function bodies are synthesized rather than checked
against their annotation, so not even a return type rescues it. An empty `Vec` is reached,
not written -- `tail` of a one-entry `Vec`, `range(0)`, a `select` nothing survives.

```toylang
fn nothing(x: Int) -> Vec<Int> = []

nothing(1)
```

```error
cannot tell what `[]` contains (at byte 33)
```

What applies to a `Vec`: the index specs (`[0]`, `[-1]`, `[]` mid-chain -- see
[specs](../operators/specs.md)), [`extent`](../builtins/extent.md),
[`concat`](../builtins/concat.md), [`tail`](../builtins/tail.md), and the subject-fed
[`select`](../builtins/select.md) and [`map`](../builtins/map.md). `+` does not join two
`Vec`s; spell that `concat([a, b])`.

Entries of a dimension need not have equal extents one dimension down; the glossary calls
such a value ragged, and collapsing an inner dimension of one yields `Opt` where entries are
missing rather than failing:

```case
index_ragged_inner
```
