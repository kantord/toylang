# Cardinality and the two layers

Every expression produces some number of values: exactly one, zero-or-one, zero-or-more.
Where that number lives is the organizing idea of the language. There are two layers, and
multiplicity can sit in either of them.

## Two layers

**Cardinality** is how many values something produces, not which ones: exactly one,
zero-or-one, zero-or-more.

**Value layer** means multiplicity is stored *in a value*. An array of three things is one
value that happens to contain three.

**Effect layer** means multiplicity is a property of *evaluation*. An expression that yields
three things is not a container; it produced three results. The name is borrowed from effect
systems, where an effect describes how an expression evaluates rather than what it returns.

The same computation can live in either layer, with the same values:

```
[1,2,3] | map(select(. > 1))     # multiplicity became array length
[1,2,3] | .[] | select(. > 1)    # multiplicity stayed in the evaluation
```

Only the location of the multiplicity differs. The second line is jq's spelling of the effect
layer; how toylang writes that crossing is [below](#reify-the-one-crossing).

In the value layer the multiplicity is invisible to the program that consumes it: `map`, `+`,
`length`, and `sort` all stay in their layer and work on one value:

```case
map_double
```

```case
filter
```

## The cardinality table

Four shapes cover the whole language:

```
T            exactly one
Opt<T>       0..1
Vec<T>       0..n, finite, indexing without iteration
Stream<T>    0..n, possibly infinite, indexing only by iteration
```

`Vec` and `Stream` are not "eager" and "lazy". The honest difference is the **promise** each
makes. `Vec` guarantees indexing without iteration; `Stream` does not, and may be infinite.
That asymmetry is load-bearing: it is why a stream must be consumed once and in order, while a
`Vec` can be indexed, sorted, and measured.

## Reify, the one crossing

Crossing from the effect layer into a value is **reify**: collect evaluation multiplicity into
a thing. In toylang that is spelled `collect`:

```case
range_basic
```

A `Stream` is born at a source -- `inputs`, `lines`, or `range` -- and dies at `collect` (or
at a sink such as `jsonlines`). After `collect` it is a `Vec` with a known extent; [the
streams guide](streams.md) and [ADR 0001](../adr/0001-stream-is-the-effect-layer-typed.md)
cover the rules that hold the stream to that single use.

The other direction, **reflect**, turns a value's contents back into evaluation multiplicity.
The design deliberately does not offer it. A `Vec` already knows its extent, so degrading it
back into a stream forgets that and buys nothing -- the multiplicity is born at sources, never
re-derived from a value. This is the one-way shift, recorded in
[Q13](../../plans/questions.md#q13-does-the-layer-shift-run-only-one-way-with-no-value-to-effect-operator)
and now the `Stream<T>` typing rule.

## The same algebra, three types

`Opt`, `Vec`, and `Stream` are one definition unpacked differently. A sequence is either
*nothing* or *one item plus a remainder*, written `1 + T*X`, where `1` is the nothing case,
`T` the item, and `X` the remainder. That template is the **base functor**:

```
Opt<T>     =  1 + T           (no remainder, so it stops after one)
Vec<T>     =  muX. 1 + T*X    (least fixpoint: must terminate, so finite)
Stream<T>  =  nuX. 1 + T*X    (greatest fixpoint: need not terminate, so possibly infinite)
```

Fixpoint means solving `X` by substituting the definition into itself. The least fixpoint
(`mu`) admits only finite solutions; the greatest (`nu`) also admits infinite ones. That single
choice is the entire difference between an array and a stream: one definition, three types, and
the finite/infinite split is *derived*, not stipulated.

## Why cardinality is the safety mechanism

jq's multi-output semantics produce hazards that all share one shape: the right feature in the
wrong position. Measured against a jq implementation:

```
if (true,false) then "a" else "b" end   -> ["a","b"]       BOTH branches executed
{} | .a = (1,2)                          -> [{a:1},{a:2}]   assignment forked the world
{a:1} | .a |= (.,.+10)                   -> [{a:1}]         |= silently took only the first
(1,2) | (., error("boom"))?              -> [1,2]           `?` truncated with no signal
```

None of these is an argument against nondeterminism: forking on assignment is config-matrix
expansion, both-branches is nondeterministic choice, and truncation under `?` is "take the
valid prefix" of a corrupt stream. What they share is that multiplicity leaked into a position
that wanted exactly one value.

Making cardinality visible turns each into a type error at the point of the mistake: `if`
requires exactly one `Bool`, a map key requires exactly one `Str`, and anything that runs an
effect requires its arguments collapsed (`first`, `only`, `collect`). Multiplicity stays free
where it is useful, notably in structural positions, where `0..n` naturally means "this many
children."

## What is still open

The settled core above leaves several threads deliberately open, tracked in the question file
rather than doctrine here: whether vectors are multidimensional ([Q9](../../plans/questions.md#q9-are-vectors-multidimensional-with--as-projection)),
whether `select` returns a masked view, a selection vector, or a copy ([Q14](../../plans/questions.md#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy)),
whether a lens ever materializes under uniqueness analysis ([Q10](../../plans/questions.md#q10-is-uniqueness-analysis-in-scope-for-deciding-when-a-lens-materializes)),
whether a dense tensor type exists ([Q17](../../plans/questions.md#q17-is-there-a-dense-tensor-type-constructed-explicitly)), and whether
the dimension model dissolves the two layers entirely ([Q32](../../plans/questions.md#q32-does-the-dimension-model-subsume-the-effect-layer)).
