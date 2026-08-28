---
type: Lesson
name: cognitive-complexity
description: What to do when clippy's cognitive_complexity fires -- look at the shape, not the number, especially in emitters
---

# cognitive-complexity

The metric counts `if`/`else`/loops/guards but NOT match arms, so the score ranks nothing
useful in this codebase: the emitters are mostly wide matches over TIR, and a perfectly
healthy emitter arm-set can outscore a genuinely tangled function. This was known and
accepted when the lint was adopted (plans/quality-practices.md, piece 4).

**Look at the shape, not the number.** Ask, in order:

1. Is the complexity a wide, flat match over TIR kinds? That is this repo's normal shape --
   the finding is inherited background, record it as inherited and move on. Do not split a
   match into meaningless pieces to appease the score.
2. Is it nested conditionals inside ONE arm doing several jobs? That is real: extract the
   arm's body into a named helper (the arm keeps one line, the helper gets the name).
3. Is it two callers sharing one body via flags? That is the fn-params lesson's territory:
   split by caller, or name the facts in a struct (see run_jq's JqInvocation).

The seven standing emitter findings at threshold 10 are all shape 1, inherited, and stay
until an emitter-split grilling decides the shared structure -- that decision is deliberately
NOT one session's improvisation, because whatever shape wins applies seven times.
