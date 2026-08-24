---
type: Note
calendar:
  - 2026-08-11
title: jq's item-wise access is the effect layer wearing brackets
description: Running the same edge cases through jq and NumPy shows that jq's brackets do not make access item-wise, the stream does, so a value-layer language cannot borrow the operator without the layer.
tags:
  - two-layer
  - projection
  - prior-art
timestamp: 2026-08-11T00:00:00Z
---

Every row below was executed, not recalled: `jq 1.8.2` and `numpy 2.5.2`, against
`[1,2,3]`, `[[1,2],[3]]`, `[[1,2,3],[4,5,6]]`, and
`{"users":[{"name":"ada","age":36},{"name":"bo","age":9}]}`.

| case | jq | result | python / numpy | result |
|---|---|---|---|---|
| every element | `[.[]]` | `[1,2,3]` | `xs` | `[1,2,3]` |
| index 1 | `.[1]` | `2` | `xs[1]` | `2` |
| filter | `[.[]\|select(.>=2)]` | `[2,3]` | comprehension | `[2,3]` |
| slice | `.[1:2]` | `[2]` | `xs[1:2]` | `[2]` |
| one field, all rows | `[.users[].name]` | `["ada","bo"]` | comprehension | `['ada','bo']` |
| field, no `[]` | `.users.name` | **error: cannot index array with string** | -- | -- |
| one row | `.users[0]` | `{"name":"ada","age":36}` | `users[0]` | same |
| one row, one field | `.users[0].name` | `"ada"` | `users[0]['name']` | `'ada'` |
| ragged, outer | `[.[]]` | `[[1,2],[3]]` | `ragged` | same |
| ragged, flatten | `[.[][]]` | `[1,2,3]` | nested comprehension | `[1,2,3]` |
| ragged, inner index 0 | `[.[][0]]` | `[1,3]` | `[x[0] for x in ragged]` | `[1,3]` |
| matrix row 1 | `.[1]` | `[4,5,6]` | `a[1]` | `[4,5,6]` |
| matrix column 1 | `[.[][1]]` | `[2,5]` | `a[:,1]` | `[2,5]` |
| rank after one index | -- | -- | `a[1].shape` | `(3,)` |
| rank after a slice | -- | -- | `a[1:2].shape` | `(1,3)` |

## What the table says

Look at `[.[][1]]` giving `[2,5]`. The second bracket is applied **per element**, while the same
bracket written directly, as in `.[1][2]`, is applied to the container. Same token, two
behaviours, and nothing in the token distinguishes them.

What distinguishes them is what came before. `.[]` produces a stream, and **everything downstream
of a stream is item-wise until something reifies it**. The brackets are not the mechanism; the
effect layer is. `[]` only looks like the item-wise operator because it is the usual way into the
layer where everything already is.

That is why `.users.name` is an error in jq: `.name` is a single-value operator, an array is a
single value, and no stream was ever created. It has nothing to do with brackets being required
and everything to do with there being no stream yet.

## The consequence for a value-layer language

jq's `[]` cannot be borrowed without borrowing the effect layer. Take the layer away, as
prototype 1 does, and the operator has no mechanism left to invoke, which is exactly what
[a pure value layer dissolves jq's iteration operators](a-pure-value-layer-dissolves-jqs-iteration-operators.md)
observed from the implementation side without knowing why.

Two mechanisms exist for a language that will not have a stream in the middle of every pipeline:

**Lifting.** Operators distribute over a collection, so `.name` on a collection of records is a
collection of names. Works on ragged data, since it only needs one level at a time. This is what
prototype 1 does.

**Per-dimension projection.** An access names what it wants from each dimension, as NumPy's
`a[:,1]` does. More expressive, since any axis is addressable directly rather than only the
outermost, but it needs rectangularity to have dimensions at all.

The two coincide on rectangular data and diverge on ragged data, where only lifting applies.
Compare `[.[][1]]` and `a[:,1]` in the table: same answer, different mental model, and only one
of them survives inner lengths that differ.

## What this says about a Frame type

A dimension is characterised by its index set: a `Vec` dimension is indexed by `0..n`, and a
named dimension by a fixed set of labels. That generalisation works, and it is what labelled-array
libraries do, but only while **the cells stay uniform**.

The moment the cells differ by label -- `name` is a `Str` and `age` is an `Int` -- the labelled
axis is not a dimension at all. It is a record, and slicing it is meaningless because there is no
common cell type to slice out. That is precisely why a data frame is a record of columns wearing
a rectangular costume, rather than a matrix.

So the split is not records versus dimensions. It is:

- uniform cells, integer labels -- a dimension
- uniform cells, name labels -- also a dimension, and a `Frame` in the sense proposed
- non-uniform cells, name labels -- a record, and not sliceable

Open: whether the uniform-cell named dimension earns its place, or whether records plus
rectangular tensors already cover every case worth having.
