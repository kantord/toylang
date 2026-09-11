**Before anything else**: run `pnpm install` in `site/` (or otherwise make a real `tsc` available,
e.g. `npm install -g typescript`, or point `$TSC` at one) so `tests/ts_types.rs`'s
`tsc_accepts_the_declaration_and_consumer` gate does not fail `just check` for reasons unrelated
to this row's own change. A prior attempt (simple_dispatch.py run 724e4eb6, 2026-09-11) reached
`just check` with a correct-looking change and failed here, then ran out of turn budget fixing it
mid-flight -- do this step first, before touching draft.md, so a turn-budget cutoff can't strand
you mid-fix again.

Board row `draft-mutation-migration`: this is only a **partial** migration -- read carefully,
do not delete more than what is actually ratified.

`draft.md` has two sections under scope, in different states:

## 1. "UNDECIDED: what to call the record-forming update" (draft.md:489-537) -- RATIFIED, retire it

This is exactly `binary-op-multiplicity-design`'s Q2/Q3, both now ratified (see that row's
title in `plans/board-archive.yaml` or `plans/board.yaml` history):

- Q3: option B -- `=` typechecks only when its right-hand side yields exactly one value
  (`One<T>`); forking to a record requires an explicit bind first
  (`("red","blue") as $c | db.color = $c`). Maintainer caveat: do NOT use a `$`-prefixed alias
  in any example/spelling -- "never agreed to prefacing variable names with $". Use a plain
  bound name instead in whatever the reference page's example becomes.
- Q2: option A -- `Vec op Vec` is cartesian by default for every binary operator (matches jq
  1.8.2's own default), no new builtin needed.

Document the settled behavior (the `One`-typed `=`, the explicit-bind-to-fork idiom, and
`Vec op Vec` cartesian default) in the appropriate reference page(s) under `reference/` --
follow the existing convention for operator/update documentation there. Verify with a small
`just check` repro that `db.color = ("red","blue")` is in fact rejected and the bind-then-assign
form works, once the feature exists; if `One`-typed `=` is not actually implemented yet, say so
plainly in the reference page (ratified design, not yet built) rather than documenting
aspirational behavior as real.

Once documented, delete the "UNDECIDED: what to call the record-forming update" subsection
(draft.md:489-537) including its options A-E list. The parent "## Mutation" section's intro
(draft.md:476-487, the cell/shadow example) stays -- it is not part of this UNDECIDED
subsection and is already-settled behavior, not a draft.

## 2. "Mutation as an optimization: privileged and shared references" (draft.md:539-570) -- NOT ratified, do NOT delete

Board's `needs: [mutation-semantics-spike, binary-op-multiplicity-design]` looks satisfied
(both rows show `status: done`), but read `mutation-semantics-spike`'s own ruling text in
`plans/board-archive.yaml`: it was a *spike* only ("will not ratify without prototyping --
spike the 'provably one reference exists' static analysis... **before a real decide row
reopens this**"). No decide row for the privileged/shared-reference mutation design itself
has ever run. `draft-split.md`'s own description of this row calls it "Hard-blocked, honestly"
pending a `mutation-semantics-design` decide row that does not exist on the board.

Do not delete this section and do not treat the spike's findings (`plans/mutation-semantics-spike.md`)
as a ratification. Instead:

- Leave draft.md:539-570 exactly as is.
- Add a new board row (append to `plans/board.yaml`, `kind: decide`, `status: todo`,
  `needs: [mutation-semantics-spike]`) named `mutation-semantics-design`, title summarizing:
  grill privileged/shared-reference mutation-as-optimization now that the spike
  (`plans/mutation-semantics-spike.md`) has grounded the "provably one reference" analysis --
  this decide row is what draft-split.md's draft-mutation-migration entry has been waiting on.
  This makes the real gap trackable instead of silently missing.

## Done-gate

- draft.md no longer contains the "UNDECIDED: what to call the record-forming update"
  subsection; its content is salvaged into the reference docs.
- draft.md still contains "Mutation as an optimization: privileged and shared references",
  unchanged.
- `just check` passes.
- A new `mutation-semantics-design` board row exists in `plans/board.yaml` (todo, decide).
- Do not board-archive `draft-mutation-migration` as fully done in the usual sense -- this is a
  partial migration. If your harness requires marking the row `done` to land, do so, but the
  new `mutation-semantics-design` row is what carries the remaining work forward; note that
  explicitly in the land commit message.
