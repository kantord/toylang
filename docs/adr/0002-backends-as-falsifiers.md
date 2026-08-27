---
status: accepted
---

# Backends are falsifiers, not a compatibility promise

Recorded 2026-08-27, well after the fact, from the repository's own notes; the decision itself
emerged across the fourth through seventh backends rather than on one day.

The language keeps seven backends (Lua, JavaScript, native/LLVM, jq, Go, Python, Rust) not to
ship on seven platforms but because each structurally unlike target falsifies checker rules the
others satisfy by accident. The admission test for a candidate backend is which axis it is
unlike the existing ones on, not whether it would be useful to have; a target that duplicates
an existing axis adds cost without evidence, and the targets are explicitly not a
compatibility promise -- not all of them will be kept.

The cost is real and accepted: every language feature is implemented seven times, and a change
to output rules touches seven emitters. What that buys, documented as it happened:

- jq, the first structurally unlike target, immediately broke two of 28 programs that three
  imperative backends had agreed on by three separate accidents
  ([a fourth backend found two rules three could not](../../research-log/a-fourth-backend-found-two-rules-three-could-not.md)).
- Go's exact constant arithmetic refused to compile an out-of-range `Int` literal that four
  backends had agreed to print unwrapped -- four accidents pointing the same way
  ([backends can agree and still be wrong](../../research-log/backends-can-agree-and-still-be-wrong.md)).
- Python found nothing, and that silence was evidence only on the two axes where Python
  differs; on every other axis it is a copy of an existing witness
  ([a backend that finds nothing is evidence only if it is different](../../research-log/a-backend-that-finds-nothing-is-evidence-only-if-it-is-different.md)).

A corollary rule governs how much a target may influence design: a target's speed constrains
the design only if that target is meant to be fast, while every target's correctness
constrains it always
([each target constrains the design differently](../../research-log/each-target-constrains-the-design-differently.md)).
