# Step 3: agreement harness

One corpus, every backend, and disagreement between backends is its own failure.

This is the thing that was going to arrive free with jaq's test corpus and now has to be built.
It is worth building at two backends rather than three, so that the third arrives into a harness
that already works.

## Shape

A corpus directory of programs, each with an optional JSON input and its expected output:

```
tests/corpus/adults.toy
tests/corpus/adults.in.json
tests/corpus/adults.out
```

Every program runs on every backend. Two distinct failures, reported differently, because they
mean different things:

- **wrong**: a backend's output does not match the expected file. The language is wrong, or the
  expectation is.
- **disagreement**: the backends produce different outputs as each other. The language is
  underspecified, and which one matches the expectation is not the point.

The second is the reason this exists. A single-backend test suite cannot express it, and it is
exactly the failure that a compiler with three targets produces.

## What goes in the corpus

Every program already living inside `tests/step_*.rs` as a positive case. Those tests keep their
own job, which is pinning error messages and emitted code per step; the corpus is about
behaviour, and behaviour is what has to be identical across targets.

Compile errors stay out. A program that does not compile never reaches a backend, so there is
nothing to disagree about.

## Do not hide a skip

If a backend cannot run because its toolchain is missing, that is a reported failure, not a
silently green run. A harness that quietly tests one backend on a machine without `node` is worse
than no harness, because the report says three.
