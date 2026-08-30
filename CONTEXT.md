# toylang

A compiled, statically typed language for transforming data, taking jq as its main inspiration
without aiming at compatibility with it. This file is the glossary and nothing else: what the
terms mean, not how anything is built. The design lives in `draft.md`, the build order in
`plans/`, and what building it taught us in `research-log/`.

## Language

### Shape of a value

**Scalar**:
A value with no interior to address: `Int`, `Str`, `Bool`.

**Record**:
A fixed set of differently-typed parts addressed by name, where the names are part of the type.
Type theory calls this a product, which is exact and says nothing to a reader who has not met the
word, so that name is not used here.
_Avoid_: object, struct, row, product

**Dimension**:
An axis along which a value repeats, addressed by position, whose entries all have the same type.
A `Vec` has one; a tensor has several. A type's dimensions are fixed by the type, in order.
_Avoid_: axis, rank, column

**Extent**:
How many entries a dimension has. A number, not a type.
_Avoid_: size, cardinality

**Field**:
One named part of a record. `name` is a field of `{name: Str, age: Int}`.
_Avoid_: column, attribute, component

**Map**:
A collection whose keys are data rather than type-level, with one value type. Distinct from a
record: a record's keys are known to the compiler, a map's are known only to the program.
_Avoid_: object, dict, record

**Name casing**:
A name beginning with a capital letter is a type; a name beginning with a lowercase letter is a
value, which covers functions and their parameters. Field names are exempt, because they come
from data and a JSON object is entitled to a key spelled `Name`.

### Building a value

**Record literal**:
The form that builds a record from its fields, and the inverse of a projection.
`{name: .n, age: .a}` is one. Its type is the names and types of its fields, so it answers what
it is from its contents alone and never needs its position to say. In argument position it is
also how a function takes more than one thing, because a function takes one argument and a record
is how several travel as one.
_Avoid_: object literal, struct literal, product literal, construction

**Vec literal**:
The form that builds a dimension from its entries. `[1, 2]` is one. Unlike a record literal it
cannot always answer what it is, because an entry is where an element type comes from and an
empty one has none.
_Avoid_: array literal, list literal

**Application**:
Calling a function with its one argument. Two spellings, one meaning: `f x`, the bare form, is
the default, and `f(x)` is the same call with the argument grouped -- the disambiguator when the
bare form would read differently (an argument starting with `-`, `.`, or `[`, a compound
argument, or a call spread across lines, since an argument must start on the same line as its
function). Both work anywhere an atom does, including as an operand. `select` and `map` are not
special syntax -- they are ordinary names reached through this same rule.
_Avoid_: call, invocation, function call

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
Choosing a field out of a record -- which part of each entry. `.name` is a projection.
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

### Choosing among shapes

**Enum**:
A declared, closed set of named variants, nominal: the name is the identity, and consuming one
must handle every variant. As data it is plain JSON, never an opaque value.
_Avoid_: union, sum type, ADT, tagged union

**Variant**:
One named alternative of an enum, optionally carrying a payload of one type -- and itself a
type, a subtype of its enum, so a signature can name it. Capitalized, per the casing rule,
because it is a type. As data: the single-key wrapper, `{"Circle": {"r": 1}}`, the name
verbatim.
_Avoid_: case, constructor, alternative

**Unit variant**:
A variant with no payload. As data: a bare string, the name verbatim (`"Active"`). Wild
lowercase string enums are no longer directly typeable; they wait for the codec layer.
_Avoid_: nullary constructor, bare tag

### Where multiplicity lives

**Stream**:
The type of effect-layer multiplicity: `Stream<T>` says an expression yields its entries one at
a time as evaluation proceeds, not that a stream object exists. Born at a source, consumed
exactly once, dead at `collect` or a sink; never inside a record, a `Vec`, or another `Stream`.
_Avoid_: lazy list, generator, iterator, channel

**Source**:
An expression a `Stream` is born from, and the only way one arises: `inputs`, `lines`.
_Avoid_: producer, reader

**Sink**:
A stream consumer legal only as a program's outermost expression, writing as it goes, with no
result type. `jsonlines` is one.
_Avoid_: printer, writer, output function

**Collect**:
The named spelling of reify at the stream boundary: `Stream<T> -> Vec<T>`, the one explicit way
a `Stream` becomes a value. Not a third layer shifter.
_Avoid_: materialize, realize, to_vec
