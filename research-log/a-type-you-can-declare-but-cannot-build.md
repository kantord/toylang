---
type: Note
calendar:
  - 2026-08-11
title: A type you can declare but cannot build
description: Record types exist in the annotation grammar and have no expression that produces one, so the only record value a program can hold arrives from input, which was found by planning a step around them.
tags:
  - type-system
  - grammar
  - prototype-1-5
timestamp: 2026-08-11T00:00:00Z
---

Prototype 1.5 step 5 was planned as "scalars, functions, and records, natively". Records could
not be done, and the reason was not that they are hard.

**There is no way to write one down.** `{` appears in the parser in type position only. The
expression grammar has a `Vec` literal and nothing for records, because object construction was
deliberately excluded from prototype 1. So `{name: Str, age: Int}` is a type a function can
declare, a value the checker will happily reason about, and a thing no program can construct.

The only record value that can exist comes from `input`. That makes records and JSON parsing one
feature rather than two, and it changed the plan: doing records in step 5 would have meant
writing a JSON parser to unlock exactly one corpus program, since the other three record
programs also need `Vec`. Both moved to step 6, where they unlock four.

The general shape is that a language has two grammars for its values -- one for describing them
and one for producing them -- and nothing forces those to cover the same ground. Type syntax grows
faster, because a type is cheap to add and an expression form is not. The gap does not show up as
a type error, because every program that survives it typechecks. It shows up much later, as a
feature that turns out to be unreachable.

Worth noticing where else this is already true. `Bool` is declarable and has no literal, so the
only Bool comes from a comparison. That one is harmless and even deliberate. The record case was
not deliberate; it was an omission that looked like a decision.

The cheap check, whenever a type is added: name the expression that produces one. If the answer
is "another part of the language", that type is not usable on its own and whatever it depends on
is really the same feature.

This is close to [checked-only forms are a class, not a lambda rule](checked-only-forms-are-a-class-not-a-lambda-rule.md),
which is about expressions that cannot state their own type. This is the reverse: a type with no
expression at all.
