---
type: Playbook
title: Naming a tuple return pushed a function over budget
description: What to do when a struct-literal rename (or similar mechanical refactor) pushes a function's own clippy too-many-lines count over budget.
tags: [too-many-lines, structure]
---

# Naming a tuple return pushed a function over budget

`check()` and `synth()`'s standing findings, and their whole history below, are now recorded as
structured exemptions in [the sinkhole](/.claude/checks/sinkhole.toml) (#54): the check script
suppresses them rather than reporting them at Stop. This lesson stays as the record of how that
was reached, and still applies as-is to any other function's inherited-vs-caused question.

`check.rs`'s `access()` returned `(Tir, Type, usize, bool)`. A review asked
for the tuple to become a named `Access` struct (readability at call sites,
not a line-count concern). Doing it the direct way -- a full `Access { .. }`
literal at every return site, and a `let Access { tir, elem, depth, stream }
= ...` destructure at every recursive call -- pushed `access()` from under
the 100-line clippy budget (it was not previously flagged) to 132/100: a
finding this session caused, not inherited.

## What settled it

Kept the named struct, shrank the boilerplate instead of reverting or
splitting the function:

- A positional `Access::new(tir, elem, depth, stream)`, matching the
  `Tir::new` convention already used throughout this file, replaces the
  struct literal at every return site.
- A recursive call's result binds to a short local (`let b = access(ctx,
  base)?;`) and is read field-by-field (`b.tir`, `b.elem`, ...) instead of
  being destructured into four names up front.

This got `access()` to 101 lines (then further edits to fit line width
brought individual lines under rustfmt's wrap point, since a name like
`base.tir` inside an already-long `Access::new(...)` call can push rustfmt
to wrap it back onto several lines, undoing the saving).

Reach for this first when a struct/enum-naming refactor is what pushed a
function over budget: the boilerplate is usually the four-line-per-field
literal, not the logic, and a positional constructor removes it without
losing the named type. Split the function instead only if there is no
"tighten the construction" way to fit -- see the *split-by-match-arm*
alternative discussed and passed over in this case, kept here as the next
thing to try if tightening does not work.

## Telling caused from inherited

`file-too-long` findings from `.claude/checks/run.sh` say whether they are
caused or inherited directly, by diffing against the merge-base blob.
`too-many-lines` findings come from clippy and carry no such label. To tell
them apart: check out the merge-base commit into a scratch `git worktree`
and run the same `cargo clippy --all-targets --message-format=json` query,
filtered to `clippy::too_many_lines`, for the same file. A function's count
there is what this branch inherited; only the gap above that is caused.

In this case, `check()` (152 at merge-base, 132 now) and `synth()` (560 at
merge-base, 543 now) were both already over budget and both shrank; neither
is a new finding, so neither was chased further here. `clippy.toml` already
documents `check.rs` as one of the three files the 100-line budget was set
to name a real conversation about, not fix incidentally -- see
`plans/quality-practices.md`.

The bare-default session (issue #20) hit the same shape from the other
direction -- a new nine-line diagnostic arm grew an already-over `synth()`
slightly -- and the same reasoning applied: growth that neither creates the
finding nor is the split conversation itself stays inherited. The
field-order session (issue #21) is a third instance: the reordered-fields
error hint grew the already-over `check()` by three lines (132 to 135), and
stayed inherited by the same rule.

The JSON-string-conformance session (issue #26) is a fourth instance, and
the first in an emitter rather than `check.rs`: dispatching `Str` ordering
through a codepoint-aware helper grew `emit_js.rs`'s already-over `expr()`
from 154 to 163 lines. Same rule, same outcome -- the same edit also grew
that file's two cognitive-complexity findings (issue #25), which this
session left standing for the same reason since that lesson does not
exist yet.

The Opt-in-the-grammar session (issue #27) is a fifth instance: its whole
diff to `check.rs` was a ten-line `resolve()` match arm, touching neither
`check()` (still 135) nor `synth()` (still 592) at all -- inherited by the
same rule, with an even more direct read since the flagged functions were
not edited.

The SoA-cursor-match session (issue #40) is a sixth instance, and the
clearest yet: the fix rewrote `emit_llvm.rs`'s `Kind::Match` arm inside
`expr()`, adding a four-line comment and turning an `if`/`else` into a
`match` -- text lines went up, but clippy's own count of `expr()` came out
at 334/100 both before and after the change (verified the same way, via a
merge-base worktree). The finding didn't just stay inherited, it was
provably unaffected by lines added elsewhere in the same function.

The type-flow session (issue #44) is the caused counterpart: promoting
`expect` to a real dispatch (`expect_inner`) and moving the match arm out
of `synth` into `match_chain` created two *new* functions over budget --
caused, not inherited, since neither name existed at merge-base. Settled
the tighten-first way: extract per-construct helpers along the seams the
file already uses (`input_read`, `inputs_read`, `variant_arm`,
`check_reachable`, `check_coverage` beside the existing `construct` and
`collect`), which put both under 100 without splitting any typing rule in
half. `check()` and `synth()` stayed flagged and stayed inherited.

The unused-variable session (issue #45) is a seventh instance: a nine-line
early-return checking the function parameter got read grew `check()` from
137 (verified via a merge-base worktree) to 146; `synth()` was untouched by
the diff and stayed at 340 both sides. Same rule, same outcome.

The check-rs-split session (issue #51) is an eighth instance, and confirms
the previous one held: extracting the type-resolution and stream-linearity
passes into `check/types.rs` and `check/linearity.rs` left `check()` and
`synth()` themselves unedited, and both still report exactly 146 and 340 --
the same counts issue #45 left them at. Moving a function's file does not
reset what counts as inherited; both stay flagged, both stay inherited.

The nullary-functions session (issue #61) is a ninth instance, and the first
where "caused" was chased rather than left standing, because a *new*
function crossing budget on day one is a shakier claim of "inherited" than a
few-line nudge to a function already over. `expect_inner` went from 99
(absent from the merge-base report) to 101 lines from two one-line optional-
argument unwraps; tightened back under budget by pulling the shared
`Expr::Call { func, arg, .. } = expr && func == "..."` guard both call sites
repeated into a `call_named()` helper, which incidentally shortened the
`map` call site enough to net a two-line saving. Separately, extracting
`synth()`'s whole `Expr::Call` arm into `call()` (see the identically-named
case in `cognitive-complexity.md`) dropped `check()` to 137 and `synth()` to
220 lines -- both still over budget and still inherited, just less so -- and
left the new `call()` itself, after its own further split into
`select_call`/`extent_call`/`tail_call`/`concat_call`, under budget with no
finding at all.
