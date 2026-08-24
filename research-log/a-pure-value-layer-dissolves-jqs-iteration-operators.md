---
type: Note
calendar:
  - 2026-08-10
title: A pure value layer dissolves jq's iteration operators
description: Building step 4 under the one-way-shift proposal left three of jq's defining operators with nothing to do, which is a real cost of the proposal rather than a refutation of it.
tags:
  - two-layer
  - cardinality
  - prototype-1
timestamp: 2026-08-10T00:00:00Z
---

Prototype 1 implements no effect layer, taking the one-way-shift proposal at its word to find
out what breaks. Step 4 was the first step where that could bite, and it did, three times.

**`[]` is the identity.** If projection by every index returns a view of the same extent, then
`[1,2,3][]` is `[1,2,3]`. The compiler agreed literally: the two spellings emitted byte-identical
Lua, and a test asserted it.

*Superseded.* This was true of the implementation, not of the design. `[]` is now a spec saying
what happens to a dimension, and a spec with no access after it is an error rather than a no-op,
so the identity claim no longer typechecks. The test that pinned it was replaced by one pinning
the error. What follows below still holds: it was the auto-distribution that emptied `[]`, and
that is what changed.

**`|` cannot be elementwise.** The step 4 plan said the load-bearing assumption was that `|`
applied to a `Vec` is elementwise. That assumption does not survive: if `|` hands `select` one
element at a time, `select` has to return zero-or-one, which is `Opt`, which is exactly the
effect-layer machinery C1 says does not exist. So `|` has to be plain composition that rebinds
`.`, and the distributing is done by the operators themselves. `select` is a whole-`Vec` mask.

**`,` has no meaning as an operator.** At the value layer, `1, 2` would build a `Vec` of two, and
`[...]` already does that. It survives in step 4 only as a separator inside a literal.

None of this refutes the proposal. Each operator behaves consistently and every program still
typechecks, which is the outcome the proposal predicts. What it shows is the *cost*: the three
things a jq user reaches for first become, respectively, a no-op, ordinary composition, and
punctuation. They get their meaning back only where extent is genuinely unknown, which under the
proposal is streaming input and nothing else.

That sharpens Q1 and Q13 rather than settling them. The question is no longer only "is a stream a
value" but "is a language where `.[]` is a no-op still recognisably in the jq family". A
defensible answer is yes, on the grounds that the work `.[]` was doing was never iteration but
extent-forgetting, and forgetting extent is what the design set out to stop.

Open, and cheap to test in prototype 2: whether reintroducing streaming input gives all three
operators non-trivial behaviour again, or only `|`.

Finding this required the emitted output to be compared, not just the program's result -- the
identity claim is invisible in what the program prints. See
[a test that cannot fail is worse than no test](a-test-that-cannot-fail-is-worse-than-no-test.md).

Why it happens, established later by running the cases through jq itself:
[jq's item-wise access is the effect layer wearing brackets](jqs-item-wise-access-is-the-effect-layer-wearing-brackets.md).

The same accounting from the other side, where a derived operation becomes primitive rather than
a primitive becoming inert:
[removing the effect layer makes map primitive](removing-the-effect-layer-makes-map-primitive.md).
