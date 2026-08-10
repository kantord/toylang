---
type: Lesson
calendar:
  - 2026-08-10
title: A second type is what makes a checker falsifiable
description: With one type in the language every argument and operand check passes by construction, so a type checker cannot be tested at all until a second type exists to violate it.
tags:
  - type-checking
  - testing
  - prototype-1
timestamp: 2026-08-10T00:00:00Z
---

The step 3 plan said "type syntax, so far only `Str`" and gave `greet(42)` as its negative case.
Those contradict, and noticing why changed the build order.

A checker with one type has nothing to reject. Every parameter is `Str`, every argument is `Str`,
and the comparison in `expect` always succeeds. The code that implements argument checking can be
written, can be read, and can be wrong in any way at all, and no test written against that
language will notice. The same held for `+`: step 2 shipped it with a comment saying the real
operand check waits for a second type, which was honest but left the check unexercised.

So `Int` moved from step 4 to step 3. Not because `Int` was needed for anything a program does at
step 3 -- there is still no arithmetic, and `Int` exists only as a literal and a type name -- but
because it is the cheapest thing that makes the checker's decisions observable. Adding it turned
three unfalsifiable code paths into three tests that can go red: argument type, return type, and
`+` operands.

The general form: the size of a type system's test surface is bounded by the number of ways a
program can be *wrong*, not by the number of ways it can be right. A language feature that only
adds valid programs adds no coverage.

The `expect` function this went through was still synthesise-then-compare at that point, doing
nothing a direct comparison would not. It earned its separate existence two steps later, when
forms turned up that can be checked but not synthesised at all:
[checked-only forms are a class, not a lambda rule](checked-only-forms-are-a-class-not-a-lambda-rule.md).

This is the same failure as [a test that cannot fail is worse than no test](a-test-that-cannot-fail-is-worse-than-no-test.md),
arriving from the language side rather than the assertion side.
