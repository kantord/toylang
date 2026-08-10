---
type: Lesson
calendar:
  - 2026-08-11
title: Merging passes turns redundant traversals into bugs
description: A double traversal that was pure waste while the checker only asked questions became a correctness bug the moment the checker also allocated, and nothing in the type system marks the difference.
tags:
  - architecture
  - compilers
  - prototype-1-5
timestamp: 2026-08-11T00:00:00Z
---

The old checker's binary-operator case walked its left operand twice: once with `synth` to find
out what type it was, and again through `expect` to verify it against `Str`. While `check` only
computed types and `lower` was a separate pass, that was pure waste. Every traversal returned the
same answer, so doing it twice cost time and nothing else.

Merging the two passes changed what a traversal *is*. The checker now allocates the local
bindings that `|` and `select` need, handing out `t_0`, `t_1` and so on as it goes. Walking a
subtree twice hands out two sets of ids for one piece of the program, and every binding after it
is renumbered. The output is still valid code, still typechecks, and is wrong.

The general form: a pass that only asks questions can be re-entered anywhere, for free. A pass
that also allocates -- ids, registers, labels, temporaries -- can be entered exactly once per
node. Merging a query pass into an allocating one silently converts every redundant traversal in
it into a correctness bug, and nothing flags them, because "call this function twice" is not an
error in any type system that will be checking this code.

So the cost of merging two passes is not the merge. It is that every existing traversal in the
absorbing pass has to be audited for whether it is entered more than once, and that audit has no
compiler support.

This one was caught while restructuring rather than by a test. What would have caught it anyway
is the acceptance criterion the step was given: byte-identical emitted output, not "the tests
still pass". Renumbered locals produce working programs, so only an assertion on the generated
code sees them, which is the same argument as
[a test that cannot fail is worse than no test](a-test-that-cannot-fail-is-worse-than-no-test.md).

Worth carrying into the LLVM backend, where the allocating passes are the norm rather than the
exception, and where the equivalent mistake produces a miscompile rather than a rename.
