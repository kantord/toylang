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

The opt-as-enum session (issue #62) hit shape 2 four times, caused: the tagged-yield choice
for partial chains landed inside `expr()`'s match arm in llvm, go, js, and lua. The llvm
instance was extracted into `opt_lit()` in-session; the other three were wrongly recorded
as inherited until the landing review measured them (go 14/10, js 16/10, lua 14/10 against
baselines 12/15/13), and were extracted into `arm_yield`/`arm_return` helpers at review.
After extraction js and lua sit at their baselines again; go carries one residual point
(13/10 vs 12/10) from the same arm's Opt re-encoding, caused and recorded here rather than
laundered into the inherited pile.

The Euler-fixtures session (issue #69) is a fourth instance, in `tests/docs.rs` rather than an
emitter: a new `fixture` fence, near-duplicating the existing `input` fence's `match &mut
pending {}` and adding a `let-else`-and-`continue` for the gated case, pushed `extract()` from
absent to 15/10. Case 2 applied the same way: the `input` and `fixture` arms' bodies (each a
three-way match on the pending slot) moved into one shared `set_pending_input()` helper, which
brought `extract()` back to absent with no finding. `every_fragment_is_a_real_program()`'s
own 11/10 was verified unchanged from merge-base throughout (checked via `git show
HEAD:tests/docs.rs` against a scratch copy) and stayed inherited by the same rule as
too-many-lines' worked examples.

The parser-floor-part-1 session (issue #75) is a fifth instance, and a variant of shape 1: a
new `Kind::Builtin` match arm per backend (`Builtin::Chars`) left `expr()`'s cognitive-complexity
unchanged in all seven backends, same as the record-reorder-through-Opt session. But two
backends' `emit()` -- `emit_js.rs` and `emit_lua.rs` -- gate their optional helper text with a
chain of separate `if used.X { out.push_str(HELPER) }` statements rather than the data-driven
`for (on, text) in [...] { if on { ... } }` loop `emit_go.rs`, `emit_py.rs`, and `emit_rs.rs`
already use for the same job; adding one more such `if` for `Builtin::Chars`'s helper pushed
`emit_js.rs`'s `emit()` from 17/10 to 18/10 and `emit_lua.rs`'s from 18/10 to 19/10 -- a real
move, not the exactly-unchanged shape-1 signature, because an `if`-chain costs the metric a
point per case where a match or a loop over a literal array costs nothing. Settled by converting
both functions' `if`-chains to the same loop pattern their three siblings already use (folding
each compound guard, e.g. Lua's `quote`/`join` conditions, into a `let` before the loop): both
functions dropped out of the findings entirely, below where they stood at merge-base, rather
than merely back to baseline. Reach for this whenever a new optional-helper gate is being added
to an `emit()` still spelled as an `if`-chain instead of the array-loop -- it is the same
tighten-first move as a match arm, just for this function's own older shape.

The Int64 session (issue #83) is a sixth instance, and names the mechanism directly: a
width-guarded *duplicate* match arm (`Kind::Arith { .. } if t.ty == Type::Int64 => ...` beside
the plain one) costs the metric per guard where a plain extra arm costs nothing, so adding one
per backend moved every emitter's `expr()` by 2-3 points (llvm and rs newly crossing). Settled
case-2 style: one unguarded arm calling a width-dispatching helper (`arith(&t.ty, op, l, r)`,
plus `int_lit` where the literal was also guarded), which put every `expr()` at or *below* its
merge-base score. One residual point stands honestly: `emit_rs.rs`'s `emit()` at 21/10 against
a 20/10 baseline, from the `wire.contains` guards that scope parser generation to
input-reachable types -- caused and recorded here, the same way go's issue-62 residual was.

The Bool-keywords session (issue #96) is a seventh instance of the same shape-1-with-one-caused
-crossing pattern, and the third time the fix was the one named above for `emit_llvm.rs`: adding
`and`/`or` as a TIR node put a forty-five-line short-circuit sequence -- an alloca, two basic
blocks and a conditional branch -- directly inside `expr()`'s new match arm, pushing that
otherwise-shape-1 dispatch from absent to 11/10. Extracting it to a `logic()` method beside the
`compare()` the arm above it already delegates to removed the finding entirely. The arm reads
`Kind::Logic { op, lhs, rhs } => self.logic(*op, lhs, rhs)?` now, exactly parallel to its
neighbour. The other six backends spell the same node as one `format!`, added no branching, and
their scores came out unchanged -- shape 1, inherited, same as every instance above.
