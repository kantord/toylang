---
type: Note
calendar:
  - 2026-08-24
title: Removing the effect layer makes map primitive
description: jq defines map as sugar for reflect-apply-reify, so a language without an effect layer cannot derive it, and the operator that was free in the inspiration becomes a builtin here.
tags:
  - two-layer
  - stdlib
  - prototype-1
timestamp: 2026-08-24T00:00:00Z
---

jq's `map` is not primitive. It is `def map(f): [ .[] | f ];` -- reflect into a stream, apply,
reify back into an array. `map(. * 2)` and `[ .[] | . * 2 ]` both give `[2,4,6]`, because they are
the same program.

Prototype 1 has no effect layer, so it has no stream to reflect into and nothing for `[...]` to
reify from. The definition has no meaning here, and `map` has to be a builtin.

This is the mirror of
[a pure value layer dissolves jq's iteration operators](a-pure-value-layer-dissolves-jqs-iteration-operators.md).
There, removing the layer made three primitives inert. Here it makes a derived operation
primitive. Both are the same accounting: the layer is where a lot of jq's expressiveness was
being stored, and taking it away moves the cost somewhere visible rather than removing it.

## What the hole actually is

Everything the language can currently do to a `Vec` either removes elements, takes one out, or
reads a named component off each:

```
[1,2,3] | select(. >= 2)   ->  [2,3]
[1,2,3][0]!                ->  1
db.users[].name            ->  the names
```

**Nothing produces a new element value.** A `Vec<Int>` can become a shorter `Vec<Int>` or a single
`Int`, and can become a `Vec<Str>` only if a `Str` was already sitting there as a component. So
the gap is not "a convenience is missing", it is that element transformation does not exist.

Fifty-two corpus programs across four backends did not notice, because every one of them is a
filter, a projection, an index or a scalar. The corpus grew along the axis the design work went
down.

## The bit that is not settled

Whether `map` stays primitive depends on something still open: if streaming input turns out to be
a dimension of unknown extent rather than a second layer, then the layer never returns and `map`
is primitive permanently. If the layer returns, `map` becomes sugar again, exactly as in jq.
Adding it now costs nothing either way, since a builtin that later becomes derived is a
definition moving from one place to another.
