Board row `draft-records-migration`: migrate the three record decisions out of `draft.md`
into the record reference, then delete them from the draft (draft-split, `plans/draft-split.md`).

Read `plans/draft-split.md`'s "draft-records-migration" entry first. Sections to retire from
`draft.md`: "DECIDED: records can be built, and a record is how several arguments travel",
"DECIDED: record fields keep their declared order", "DECIDED: record field order is not type
identity". Destination: `reference/types/record.md`, with the unary-functions/record-argument
story also touching `reference/syntax/functions.md` (shared ground with
draft-access-model-migration -- reconcile rather than duplicate if that page already covers it).
The punning refusal and its stated reason are load-bearing rationale and must survive the move.

Verify each destination page still matches the current implementation (`just check` a small
repro if anything looks stale) before folding in anything missing. Once the three sections'
content is confirmed covered, delete those three sections from `draft.md`. Done-gate:
`draft.md` no longer contains the three sections, `reference/types/record.md` (and
`reference/syntax/functions.md` if touched) reflect the salvaged rationale, and `just check`
passes.
