# toylang

A compiled, statically typed language for transforming data, taking jq as its main inspiration
without aiming at compatibility with it. This file is the glossary and nothing else: what the
terms mean, not how anything is built. The design lives in `draft.md`, the build order in
`plans/`, and what building it taught us in `research-log/`.

## Language

### Shape of a value

**Scalar**:
A value with no interior to address: `Int`, `Str`, `Bool`.

**Product**:
A fixed set of differently-typed parts addressed by name, where the names are part of the type.
A record is a product.
_Avoid_: object, struct, row

**Dimension**:
An axis along which a value repeats, addressed by position, whose entries all have the same type.
A `Vec` has one; a tensor has several. A type's dimensions are fixed by the type, in order.
_Avoid_: axis, rank, column

**Extent**:
How many entries a dimension has. A number, not a type.
_Avoid_: length, size, cardinality

**Component**:
One named part of a product. `name` is a component of `{name: Str, age: Int}`.
_Avoid_: field, column, attribute

**Map**:
A collection whose keys are data rather than type-level, with one value type. Distinct from a
product: a product's keys are known to the compiler, a map's are known only to the program.
_Avoid_: object, dict, record

### Building a value

**Product literal**:
The form that builds a product from its components, and the inverse of a projection.
`{name: .n, age: .a}` is one. Its type is the names and types of its components, so it answers
what it is from its contents alone and never needs its position to say. In argument position it
is also how a function takes more than one thing, because a function takes one argument and a
product is how several travel as one.
_Avoid_: object literal, struct literal, construction

**Vec literal**:
The form that builds a dimension from its entries. `[1, 2]` is one. Unlike a product literal it
cannot always answer what it is, because an entry is where an element type comes from and an
empty one has none.
_Avoid_: array literal, list literal

### Reaching into a value

**Spec**:
What an access says about one dimension: keep it, narrow it, or collapse it. Every dimension of a
value needs one, which is why `[]` is written rather than assumed.

**Keep**:
A spec that leaves a dimension at full extent, written `[]`. Streamable, since it consumes
nothing.

**Narrow**:
A spec that leaves a dimension at reduced extent, such as a mask. Streamable.

**Collapse**:
A spec that removes a dimension, such as an index. Not streamable: it has to consume to find the
entry, so on a stream it destroys what it passed.

**Selection**:
Choosing along a dimension -- which entries. `select` and index specs are selections.
_Avoid_: filter, projection

**Projection**:
Choosing a component out of a product -- which part of each entry. `.name` is a projection.
_Avoid_: select, field access, column selection

### Guarantees

**Rectangular**:
Every entry of a dimension has the same extent in the next dimension down. A refinement, not a
requirement: it is what makes collapsing an inner dimension total, so a non-rectangular value
yields `Opt` where entries are missing rather than being inaccessible.
_Avoid_: dense, uniform, matrix-shaped

**Ragged**:
Not rectangular. Dimensions are still enumerable and still addressable; only totality is lost.
_Avoid_: jagged, irregular

**Enumerable dimensions**:
A type fixes how deep it goes, so its dimensions can be named in order. `Vec<Vec<Int>>` has two.
A type of unbounded depth has none, and is reached only by recursive descent.
