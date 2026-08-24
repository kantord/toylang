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

The reverse move is what makes a refactor safe: prototype 1.5 step 1 was given "byte-identical
emitted output" as its acceptance rather than "the tests pass", because the bug it was most
likely to introduce produces working programs. See
[merging passes turns redundant traversals into bugs](merging-passes-turns-redundant-traversals-into-bugs.md).

The check that catches this is to ask what change would make the test red, and to name it
concretely. Then actually break it and watch, because **the verification is itself a test that
can silently not fail**, and it did, twice in one sitting.

Checking the step 3 agreement harness meant deliberately introducing each failure it claims to
catch. Two of the three attempts were no-ops that looked like passes:

- The patch meant to make one backend disagree edited `parts.join(",")`, which in the Rust source
  is written `parts.join(\",\")` inside a string literal. The pattern matched nothing, the
  backends still agreed, and the harness stayed green. Green was the correct answer to a question
  that had not been asked.
- The attempt to prove a missing toolchain is reported removed one `node` from `PATH`. There was
  a second one at `/usr/bin/node`. Again green, again meaningless.

Both produce the same output as a real pass, which is the entire problem. What settled it was
checking the setup rather than the result: printing the patched line to confirm the edit landed,
and running `command -v node` to confirm it was gone. Only then did the failures appear, and all
three modes did fire.

So "I verified the test can fail" is a claim that needs its own evidence, and the evidence is
seeing the failure text, not seeing the suite go red-then-green.

The coverage version of the same question -- not "can this assertion fail" but "would anyone
notice if this were never checked" -- is
[track an incomplete backend as a shrinking list](track-an-incomplete-backend-as-a-shrinking-list.md). If the answer is "none", the test is documentation with a green tick next to it,
which is worse than no test because it is counted as coverage.

This is worth watching for specifically in a compiler, where the same output can be produced by
many different internal structures, and where the interesting properties live in the structure.
The same move paid off at step 4: that `[]` is a no-op is invisible in what a program prints and
provable by comparing emitted code, which is how
[a pure value layer dissolves jq's iteration operators](a-pure-value-layer-dissolves-jqs-iteration-operators.md)
got its evidence.
[The backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md)
is the other side of the same problem: there, a property that was not observed turned out to be
an actual bug.

The same failure arriving through a fixture's configuration rather than through an assertion is
[a config field that is ignored is a check that did not run](a-config-field-that-is-ignored-is-a-check-that-did-not-run.md).
