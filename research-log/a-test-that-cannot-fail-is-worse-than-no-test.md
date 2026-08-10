---
type: Lesson
calendar:
  - 2026-08-10
title: A test that cannot fail is worse than no test
description: Two assertions in prototype 1 could never have gone red, because the property each claimed was invisible in what it observed. Assert on the structure that carries the property instead.
tags:
  - testing
  - prototype-1
timestamp: 2026-08-10T00:00:00Z
---

Twice in three steps, a test was written whose stated claim could not have failed.

Step 2 asserted that `+` is left-associative by running `"a" + "b" + "c" + "d"` and checking the
output. But `+` on strings is associative and the emitter flattens the chain, so both
associativities produce `abcd`. The assertion was real, the claim in the comment above it was
not, and no bug in the precedence table would have turned it red. The fix was to snapshot the
parse tree, which is the only place the nesting is observable.

Step 3 planned `greet(42)` as its negative case while the language had only `Str`. Every argument
would have been `Str` and every parameter would have been `Str`, so the check passed by
construction. See [a second type is what makes a checker falsifiable](a-second-type-is-what-makes-a-checker-falsifiable.md).

Both have the same shape and it is not "we forgot to test the failure case". The test existed,
ran, and was green. What was missing is that the *observation* did not carry the property being
claimed. Output is a lossy view of the program: it discards associativity, it discards which of
two equal types was involved, and it will discard evaluation order.

The check that catches this is to ask what change would make the test red, and to name it
concretely. If the answer is "none", the test is documentation with a green tick next to it,
which is worse than no test because it is counted as coverage.

This is worth watching for specifically in a compiler, where the same output can be produced by
many different internal structures, and where the interesting properties live in the structure.
[The backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md)
is the other side of the same problem: there, a property that was not observed turned out to be
an actual bug.
