---
type: Note
calendar:
  - 2026-08-10
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
