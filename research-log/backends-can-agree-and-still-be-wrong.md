---
type: Lesson
calendar:
  - 2026-08-24
title: Backends can agree and still be wrong
description: An out-of-range Int literal printed identically on four backends, so the agreement harness stayed green, but each of them avoided the rule by its own accident rather than by following it.
tags:
  - agreement-harness
  - backends
  - testing
timestamp: 2026-08-24T00:00:00Z
---

`str(9999999999)` printed `9999999999` on Lua, JavaScript, jq and the native backend. Four
backends, one answer, no disagreement to report. The answer was wrong: `Int` is 32 bits and
wraps, and that number is not one.

They agreed because each of them held the literal in its own wider representation -- a Lua
integer, a double, jq's preserved number text, an `i64` -- and only wrapped once an operator
touched it. Four independent accidents, all pointing the same way. The harness is built to catch
one backend behaving differently, and here nothing did.

**The literal was the one place a value could enter without meeting its type.** Every operator
had been made to wrap; the entry point had not. That is a general shape worth watching for: a
rule enforced at every edge and never at the source.

Go found it by refusing to compile. Its constant arithmetic is exact and unbounded, and a typed
constant that does not fit is a compile error, so `int32(2147483647) + int32(1)` does not build.
The check now lives in the checker instead, where it belongs: an `Int` literal must fit in 32
bits, and a minus directly on a literal is part of the literal, so `-2147483648` is writable and
`-2147483649` is not.

## What this says about the harness

The agreement harness proves that the backends say the same thing. It cannot prove that the thing
is what the language means, and the gap is widest exactly where the backends are *similar*.
Lua, JavaScript, jq and native all store an integer in something at least 53 bits wide, so on
this question they were never four witnesses. They were one witness, quadrupled.

That is the same argument as
[a fourth backend found two rules three could not](a-fourth-backend-found-two-rules-three-could-not.md),
arriving from the opposite direction. There, three targets satisfied a front-end rule by three
different accidents and a structurally unlike fourth could not. Here, four targets *violated* a
rule by four different accidents and a stricter fifth would not. In both cases what a new backend
bought was not coverage but independence.

The related failure, where the observation does not carry the property being claimed, is
[a test that cannot fail is worse than no test](a-test-that-cannot-fail-is-worse-than-no-test.md).
This one is worse in a specific way: the observation carried the property fine, and every backend
reported it. There was simply nothing in the suite that knew what the right answer was, because
`.out` files record what the language does rather than what it should do. The corpus and the
harness together check consistency; conformance still has to be stated somewhere by hand.

See also
[losing jaq's corpus means building the agreement harness](losing-jaqs-corpus-means-building-the-agreement-harness.md),
which is where the harness came from, and
[a statically typed target asks for the types the checker already has](a-statically-typed-target-asks-for-the-types-the-checker-already-has.md)
for what else the fifth backend turned up.

The positive form of the same rule -- when a backend's agreement *is* evidence -- is
[a backend that finds nothing is evidence only if it is different](a-backend-that-finds-nothing-is-evidence-only-if-it-is-different.md).

A sixth instance of the same shape, found by testing a carriage return that nothing before it
had exercised: [a sixth instance of the backend having rules the checker does not](a-sixth-backend-rule-the-checker-did-not-know.md).
