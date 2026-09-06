Board row `draft-access-model-migration`: verify unary functions, lens access, and the
dimension-spec model against the reference pages; salvage rationale, delete sections
(draft-split, plans/draft-split.md).

Read `plans/draft-split.md`'s "draft-access-model-migration" entry first for the exact
sections and destination pages. Sections to retire from `draft.md`: "Functions are unary",
"Field access is a lens", "PROPOSAL: every dimension gets a spec". These are largely already
built and documented at `reference/syntax/functions.md`, `reference/operators/specs.md`,
`projection.md`, and `unwrap.md` -- verify each page still matches the current implementation
(`just check` a small repro if anything looks stale), then salvage any missing rationale into
those pages (the bidirectional-checking argument for lenses, the spec vocabulary's
derivations). The lens trait sketch (`set`, `path` -- write and path-witness halves) is
unbuilt future design, not documented behavior: push it to `plans/questions.md` as a new
numbered question rather than documenting it as real.

Once the three sections' content is confirmed covered (docs updated where something was
missing) and any open threads are moved to the tracker, delete those three sections from
`draft.md`. Done-gate: `draft.md` no longer contains the three sections, the destination docs
pages reflect any salvaged rationale, `just check` passes, and anything not yet built is in
`plans/questions.md` instead of left in prose.
