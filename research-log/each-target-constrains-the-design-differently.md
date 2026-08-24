---
type: Technique
calendar:
  - 2026-08-24
title: Each target constrains the design differently
description: Deciding how integers work turned on a rule worth keeping: a target's speed constrains the design only if that target is meant to be fast, while every target's correctness constrains it always.
tags:
  - backends
  - performance
  - decision-making
timestamp: 2026-08-24T00:00:00Z
---

Choosing what `Int` means took several reversals, and every one of them came from applying the
same standard to all four backends. The rule that ended the argument:

> A target's **speed** constrains the design only if that target is meant to be fast. A target's
> **correctness** constrains it always.

Applied, the four are not four of a kind:

| target | what it is for | what it constrains |
|---|---|---|
| native, through LLVM | being fast | both. Hardware semantics are the reference: wrapping is free, trapping is a branch. |
| Lua 5.4 | embedding | both, mildly. It has real i64 integers, so it is in the exact camp rather than the doubles camp. |
| node, through V8 | being fast where a JIT is what you have | both, and **not by instruction count**. |
| jq | agreeing, and being recognisable | correctness only. |

## The V8 entry is the one that is easy to get wrong

Costing node in instructions is the mistake. V8's fast path is the Smi, which is a 32-bit
integer; a value outside that range becomes a heap number, which is a *representation* change
rather than an extra operation. So a design whose default integer does not fit a Smi has not
added a compare to node, it has moved every value off the fast path.

That is why a 53-bit integer -- exactly representable in a double, and therefore looking free on
paper -- is worse on node than a 32-bit one that needs an explicit `|0`. The `|0` disappears once
the JIT has type feedback. The heap number does not.

## The jq entry is the one that is easy to over-weight

jq boxes and refcounts every value, so nothing emitted for it is fast and no amount of care will
change that. Letting its performance influence the design buys nothing. Its *correctness* is
worth a great deal, because it is the target that is structurally unlike the others and therefore
the one that finds rules the others satisfy by accident.

Concretely: wrapping 32-bit multiplication has no direct spelling in jq, since the true record
of two 32-bit numbers needs 62 bits and a double holds 53. It is still exactly implementable by
splitting into 16-bit halves, at about five operations. Under the rule, five operations on jq is
free, and the same five on native would not be.

## What went wrong before the rule was explicit

Three recommendations in a row, each reversed by a fact rather than an argument. i64 with
trapping, until the check turned out to be an intrinsic rather than a compare. Then 53-bit
checked, until V8's Smi boundary turned out to matter more than the compare. Then 32-bit
wrapping, which is where the rule and the measurements finally agreed.

The pattern in the mistakes is that each one weighed all four targets equally. They are not equal,
and saying how they differ *before* comparing options is what made the last comparison hold.

Related, from the other direction:
[a fourth backend found two rules three could not](a-fourth-backend-found-two-rules-three-could-not.md).
