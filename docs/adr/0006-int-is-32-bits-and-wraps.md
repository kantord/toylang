---
status: accepted
---

# Int is a signed 32-bit integer, and arithmetic wraps

Recorded 2026-08-27, after the fact. The full argument lives in draft.md's
[DECIDED: Int is 32 bits and wraps](../../draft.md#decided-int-is-32-bits-and-wraps) and is not
duplicated here; this ADR exists so the decision is findable next to the others.

The default integer diverges from jq, which has only IEEE doubles. It was settled by
measurement after three recommendations were each reversed by a fact (i64 with trapping, then
53-bit checked, then 32-bit wrapping), under the rule that ended the argument: a target's
speed constrains the design only if that target is meant to be fast, while every target's
correctness constrains it always. 32 bits keeps every value on V8's Smi fast path, is free on
native and Lua, and is exactly emulable on jq; wrapping rather than trapping because a branch
is a side effect and blocks vectorization. Division truncates; a zero divisor is the only
arithmetic failure; `MIN / -1` wraps like everything else. The named cost: millisecond
timestamps do not fit, accepted as a loud validator failure rather than silent corruption,
with a second integer type left as an open question.

Sources with the measurements and the process:
[each target constrains the design differently](../../research-log/each-target-constrains-the-design-differently.md),
[backends can agree and still be wrong](../../research-log/backends-can-agree-and-still-be-wrong.md)
(the literal that had never met its type, found by Go).
