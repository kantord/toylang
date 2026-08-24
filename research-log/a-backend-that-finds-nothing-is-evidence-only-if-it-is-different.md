---
type: Lesson
calendar:
  - 2026-08-24
title: A backend that finds nothing is evidence only if it is different
description: Python ported 68 corpus programs with no front-end change at all, which is worth something only because it fails unlike the five already there, and would be worth nothing from a sixth imperative dynamic language.
tags:
  - backends
  - python
  - agreement-harness
timestamp: 2026-08-24T00:00:00Z
---

Go arrived and immediately found a hole: an `Int` literal that had never met its type. Python
arrived and found nothing. All 68 corpus programs agreed on the first run, and no line of the
lexer, parser or checker changed.

The temptation is to report that as a stronger result than Go's. It is a weaker one, and it is
only worth anything at all because of *how* Python differs.

**Where it is unlike the others.** Its integers are exact and unbounded, so the 32-bit rule is
entirely emulated -- jq is the only other target in that bucket. Its `//` and `%` are floored, so
truncated division has to be written out -- Lua is the only other target in that bucket. It is
the first target in **both** buckets at once, and the useful finding is that the two emulations
do not compound: wrapping is one modulo, because exact integers lose nothing on the way, and
truncated division is a sign fixup over `//`. jq's expensive case was the multiply, where a
double drops the low bits of a 62-bit record and the fix is a split into 16-bit halves. Python
needs no such thing.

**Where its silence means nothing.** Python is dynamically typed, so the depth-polymorphic
`tl_field(v, k, depth)` that Go could not have works here exactly as it does in Lua and
JavaScript. On that question Python is not a fourth witness. It is Lua again.

That is the shape of the rule. A backend's agreement is evidence about the type model only on
the axes where it is genuinely unlike the ones already present. On every other axis it is a
copy, and counting it is the same error as
[backends can agree and still be wrong](backends-can-agree-and-still-be-wrong.md), where four
targets said the same wrong thing because on that one question they were one target, quadrupled.

## What the exercise is for

These targets are not a compatibility promise and not all of them will be kept. A language that
compiles easily to one ecosystem and not to another has usually made a type decision without
noticing, and trying is the cheapest way to find it. So the question for a candidate backend is
not whether it would be useful to have, but which axis it is unlike the others on.

By that test a seventh imperative dynamic language earns nothing. What is still unrepresented is
a target with a foreign string model, which is
[the string representation question](../draft.md#q16-string-representation-given-wtf-16-on-the-js-target)
waiting for a target that forces it, and a streaming target, which is the one that would reshape
the design rather than extend it.

## What the probing found instead

Adding a backend also means asking what the corpus never covered, and two gaps turned up that
had nothing to do with Python. Every backend writes its own JSON quoter and none had been given
anything but ASCII; six of them now agree byte for byte on astral-plane characters, escapes and
a raw control byte, through six string representations and six JSON parsers. And absence had
never been tested against a present-but-empty `Vec`, which is the one place Python could have
gone wrong on its own, since `None` and `[]` are both falsy there and it uses `is None`.

Both are now corpus programs. The expectations were validated against an independent JSON parser
rather than recorded from the compiler, which is the difference between a test and a transcript.
See [a test that cannot fail is worse than no test](a-test-that-cannot-fail-is-worse-than-no-test.md).
