---
status: accepted
---

# The corpus is the spec, and it checks consistency, not conformance

Recorded 2026-08-27, after the fact, from AGENTS.md and the research log; the harness predates
most of the backends.

The language's behavioral record is a corpus of YAML cases (`tests/corpus/`), one file per
case: a program, optional input, and either the expected output or `refuses: true`. Every case
runs on every backend, and the test is agreement -- all backends print the same bytes, or all
refuse (each in its own words). There is no per-backend test suite and no reference
implementation; the corpus plus the agreement requirement is what "correct" means day to day.
Snapshots exist as a deliberate exception for claims output cannot carry, and programs that do
not compile live in `step_*.rs` instead, since nothing about them differs per backend.

Two limitations were discovered rather than designed, and both are load-bearing enough to
record:

- Agreement proves the backends say the same thing, not that the thing is what the language
  means. The gap is widest where backends are similar: four targets once agreed on a wrong
  answer because on that question they were one witness, quadrupled
  ([backends can agree and still be wrong](../../research-log/backends-can-agree-and-still-be-wrong.md)).
  Conformance has to be stated by hand; expectations are validated against independent tools,
  not recorded from the compiler
  ([a test that cannot fail is worse than no test](../../research-log/a-test-that-cannot-fail-is-worse-than-no-test.md)).
- Output equality cannot see behavior. A fused streaming loop and a fully eager program print
  identical bytes on finite input, so streaming needed its own liveness probes
  (`tests/streaming.rs`), and checker-level rules need their own rejection tests. Any future
  property that is not a function of the output needs its own test kind; the corpus will stay
  green while it regresses.
