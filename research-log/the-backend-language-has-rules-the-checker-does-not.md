---
type: Note
calendar:
  - 2026-08-10
title: The backend language has rules the checker does not
description: Two prototype 1 bugs came from the emitted Lua rather than from toylang, one from Lua's local scoping and one from its global namespace, and neither was visible anywhere in the front end.
tags:
  - backends
  - lua
  - prototype-1
timestamp: 2026-08-10T00:00:00Z
---

Step 3 shipped two defects that had nothing to do with toylang's semantics.

The checker collects every signature before checking any body, so a definition may call one that
appears further down the file. That is a deliberate rule and it is tested. The emitter wrote
`local function` in source order, and in Lua a `local` is not in scope until after its statement,
so the forward call resolved to a global, found nil, and died at runtime. The program typechecked
and then crashed. Fixed by declaring every name before any body, which also covers mutual
recursion whenever it arrives.

The second: a toylang function may legitimately be called `print`. Emitted verbatim, it would
shadow the host `print` that the runner captures output through. Every emitted name is now
prefixed, because the target's namespace is not ours to spend.

Both are the same kind of thing. The front end has a model of what is legal, and the target has
its own, and nothing in the pipeline compares them. There is no type error to raise, because the
type system is not wrong; the lowering is.

Three consequences worth carrying forward.

Each new backend re-opens this. Lua's scoping rule, JavaScript's hoisting rule, and a native
backend's linkage rules are three different answers to "when is a name visible", and a front-end
rule that quietly relies on one of them will break on the others. This is an argument for
[building the cross-backend agreement harness](losing-jaqs-corpus-means-building-the-agreement-harness.md)
earlier than it feels necessary.

The bug was found by a test, not by reading, and only because the test ran the program rather
than inspecting the emitted source. A snapshot of the generated Lua would have looked correct.
That is the inverse of [a test that cannot fail](a-test-that-cannot-fail-is-worse-than-no-test.md):
here the property was observable only in the strongest observation available.

Expect more of this once streams lower to something other than a counted loop, since that is
where the three backends diverge most. Step 5 found the mirror image, where the checker holds
something the backend needs:
[the lowering needs types the checker already computed](the-lowering-needs-types-the-checker-already-computed.md).

A jq backend later supplied the third instance, and the sharpest: forward references, which Lua
satisfied with declarations and JavaScript with hoisting, cannot be expressed there at all. See
[a fourth backend found two rules three could not](a-fourth-backend-found-two-rules-three-could-not.md).

A sixth instance, the first found by a target silently disagreeing rather than refusing to
compile or run: [a sixth instance of the backend having rules the checker does not](a-sixth-backend-rule-the-checker-did-not-know.md).
