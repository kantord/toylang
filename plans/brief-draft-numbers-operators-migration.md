Board row `draft-numbers-operators-migration`: verify the Int/Float/equality sections
against ADRs 0006/0007/0010 and the reference; move what's missing, delete
(draft-split, plans/draft-split.md).

Read `plans/draft-split.md`'s "draft-numbers-operators-migration" entry first for the exact
sections and destination pages. Sections to retire from `draft.md`: "DECIDED: Int is 32 bits
and wraps" (including its conditional-expression, output/unlines, and six-backends
subsections), "DECIDED: Float is JavaScript's double", "DECIDED: equality on a composite is
structural, and stops at a Vec". This is the best-covered cluster -- ADRs 0006, 0007, 0010
plus `reference/types/int.md`, `int64.md`, `operators/arithmetic.md`, `conditional.md`,
`comparison.md`, and ADR 0002 for the backend-audit framing already document most of this.
Mostly a verify-and-delete row: check each destination page still matches the current
implementation (`just check` a small repro if anything looks stale). Anything the ADRs lack
(the literal-width rule's Go story, the ordering-still-disagrees caveat on composites) moves
into the matching reference page first, not left only in `draft.md`.

Once the three sections' content is confirmed covered (docs updated where something was
missing) and any open threads are moved to the tracker, delete those three sections from
`draft.md`. Done-gate: `draft.md` no longer contains the three sections, the destination docs
pages reflect any salvaged rationale, `just check` passes, and anything not yet built is in
`plans/questions.md` instead of left in prose.
