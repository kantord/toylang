---
type: Note
calendar:
  - 2026-08-24
title: A fourth backend found two rules three could not
description: Compiling to jq surfaced a scoping rule and an output rule that Lua, JavaScript and LLVM all happened to satisfy, which is the argument for a target that is structurally unlike the others.
tags:
  - backends
  - jq
  - agreement-harness
timestamp: 2026-08-24T00:00:00Z
---

Three backends agreed on 28 programs. Adding a fourth broke two of them immediately, and neither
break was in the new backend's code.

**Definition order.** The checker collects every signature before checking any body, so a
definition may call one that appears further down. Lua needed forward declarations to honour
that and got them at prototype 1. JavaScript hoists, so it needed nothing. LLVM declares before
defining anyway. jq resolves a `def` only against what is already defined and **has no forward
declaration at all**, so the definitions have to come out callee-first. Three targets satisfied a
front-end rule by three different accidents, and the fourth could not.

**When output is raw.** A top-level `Str` prints raw and everything else prints as JSON. Lua,
JavaScript and the native backend implement that from the type, because none of them has any
other option. jq has `-r`, which does it from the *runtime value*, so `["ada","bo"][0]` printed
`ada` where the others printed `"ada"`.

The second is the more interesting failure, because jq's answer is not merely different, it is
inconsistent: with `-r`, a present `Opt<Str>` prints raw and an absent one prints the bare word
`null`. Two shapes of output from one expression, decided by data. Reaching for the convenient
flag would have imported that, so the flag is used only when the program's type is exactly `Str`.

## What the exercise was actually worth

A backend that is structurally unlike the others earns its place by failing differently. Lua,
JavaScript and native are all imperative, and their agreement on a front-end rule was three
coincidences rather than evidence. jq is a stream language, and compiling to it meant saying in
stream terms what the design deliberately says in dimension terms: a spec that keeps a dimension
becomes `.[]` plus a reification, since keeping a dimension where everything is a stream means
iterating and collecting.

One mapping came out better than expected. `Opt` becomes null, and that is lossless **in this
direction only**: toylang has no null value, so an absent entry is the one thing a null can mean.
The conflation that makes jq lose information is harmless when translating into it.

This is the third instance of
[the backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md),
and the first where a rule the checker relies on was unsatisfiable rather than merely
unimplemented.
