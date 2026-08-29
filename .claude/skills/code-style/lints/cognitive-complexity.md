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

The nullary-functions session (issue #61) hit shape 2 twice, both genuinely caused (absent at
merge-base): wrapping two existing top-level `if`s in `check()` inside one new `if let Some(param)
= &def.param` added a nesting level neither had before, and `synth()`'s already-large
`Expr::Call` arm -- already doing several jobs (`select`/`map`/builtins/user-function dispatch)
-- picked up several new `?`-early-returns for the new optional-argument checks, pushing it from
absent to 58/10. Both settled the case-2 way: `check()`'s two checks moved into `check_param()`,
called from one `if let` instead of two nested `if`s; `synth()`'s whole `Call` arm moved into its
own `call()` function (further split into `select_call`/`extent_call`/`tail_call`/`concat_call`,
alongside the pre-existing `map_call`/`collect`), which also cut `synth()`'s too-many-lines count
from 340 to 220 lines as a side effect. `call()` itself lands under both budgets after the split,
the same as `map_call` and `collect` already did -- one helper per builtin form is this file's
existing shape, not a new one invented to dodge the score. A third instance, `emit_llvm.rs`'s
`expr()`, is the same pattern in a shape-1 function: a new two-armed `match arg {}` for the
optional call argument added just enough nesting to cross from absent to 11/10; moving it into
`call_args()` fixed it the same way, and is the general answer whenever a small new match
pushes an otherwise-shape-1 dispatch over threshold -- try the tighten first, same as
too-many-lines' identically-named playbook.

The Euler-fixtures session (issue #69) is a fourth instance, in `tests/docs.rs` rather than an
emitter: a new `fixture` fence, near-duplicating the existing `input` fence's `match &mut
pending {}` and adding a `let-else`-and-`continue` for the gated case, pushed `extract()` from
absent to 15/10. Case 2 applied the same way: the `input` and `fixture` arms' bodies (each a
three-way match on the pending slot) moved into one shared `set_pending_input()` helper, which
brought `extract()` back to absent with no finding. `every_fragment_is_a_real_program()`'s
own 11/10 was verified unchanged from merge-base throughout (checked via `git show
HEAD:tests/docs.rs` against a scratch copy) and stayed inherited by the same rule as
too-many-lines' worked examples.
