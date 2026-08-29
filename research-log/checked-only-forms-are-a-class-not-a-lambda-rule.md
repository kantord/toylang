---
type: Note
calendar:
  - 2026-08-10
  - 2026-08-29
  - 2026-08-30
title: Checked-only forms are a class, not a lambda rule
description: The draft states the annotation rule as being about lambdas, but prototype 1 found two more forms that can only be checked and never synthesised, so the rule is about a class of expression.
tags:
  - type-checking
  - bidirectional
  - prototype-1
timestamp: 2026-08-10T00:00:00Z
---

The draft presents bidirectional checking through one example: lambdas never annotate, because a
lambda only ever appears where something already knows what it wants. Prototype 1 turned up two
more expressions with exactly that property, and neither is a lambda.

`input` has no type of its own. What stdin contains is a runtime fact, so the only thing that can
give it a type is the position it appears in -- here, the parameter type of the function it is
passed to. Used where nothing expects anything, it is an error rather than a guess:

```
input                       # ERROR: cannot tell what `input` contains
```

An empty `Vec` literal is the same shape. `[]` has no element type to synthesise and nothing in
prototype 1 supplies one, so it errors identically.

Three instances is enough to say the rule is not "lambdas are special". It is that some
expressions denote a *shape* rather than a value with a type, and the type has to arrive from
outside. Synthesis and checking are not two conveniences; they are two genuinely different
questions, and a form can be answerable in one and not the other.

Worth noting what this predicts. Every future construct of this kind gets the same treatment for
free -- an empty record, a `null` whose type is unknown, a numeric literal once there is more
than one numeric type. Each will need an expected type or an error, and none of them will need a
new rule.

It also means the error message matters more than it seems. "Cannot tell what `input` contains"
is the whole of what the user needs to know, and the fix is always the same: put it somewhere
that says. That is a much better failure than the inference guess the draft is arguing against,
which fails somewhere else entirely.

The mirror image, a type with no expression that produces one rather than an expression that
cannot state its type, is
[a type you can declare but cannot build](a-type-you-can-declare-but-cannot-build.md).

Related, because the machinery is shared:
[the lowering needs types the checker already computed](the-lowering-needs-types-the-checker-already-computed.md)
is about the same checker handing what it learned to the backend.

## Closed by the type-flow rework (2026-08-29)

The rework `plans/type-flow.md` planned has landed, and the class held up. The positions that push
an expectation grew from one (`input`'s) to the whole surface: a declared return type flows
into the function body, a record type into its fields, a parameter type into its argument, an
expected element through a `map` body, and the expectation into both conditional branches and
every arm of a total match chain. `[]` resolves in all of them, and the prediction about future
members came true on schedule: a string literal against an enum type joined the class (the unit
variant it names), needing an `expect` arm and no new rule.

Two boundaries were worth keeping. Expectation only resolves what synthesis refused -- a form
that can answer for itself is compared, never coerced -- and a partial guard chain's arms still
synthesise, because the arms-are-already-Opt refusal depends on asking the arms what they are
(issue #48 records the fork). The map-body half of the story closes in
[a map body cannot infer from what consumes it](a-map-body-cannot-infer-from-what-consumes-it.md).

## The second boundary moved (2026-08-30)

The arms-are-already-Opt refusal was the reason the second boundary held, and #62 (tagged
absence, `Opt` an ordinary generic enum) deleted that refusal: an arm that is itself
`Opt`-typed just doubles the wrapping now, so nothing was left asking the arms what they are
before the expectation could reach them. #74 pushes a declared `Opt<T>` return through the
peel, `T`, the same way every other position pushes its expectation down. The class this note
opened stayed a class; the boundary was downstream of a rule that already fell.
