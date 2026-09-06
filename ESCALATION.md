# Escalation: the four open threads were already tracked, and the tracker is off-limits

## Question

Step 4 of the brief says to "move the embedded open-question threads (multidimensional
vectors, tensor/rectangularity, copy-on-write, select-copy) to plans/questions.md as new
numbered questions." The hard constraints also say "do not touch plans/". These would conflict
only if those threads were not already in the tracker.

## The state that settles the conflict

All four threads already live in `plans/questions.md`, under exactly the numbers the plan
(`plans/draft-split.md`) names for them:

- multidimensional vectors -> Q9
- tensor/rectangularity -> Q17
- copy-on-write -> Q10
- select-copy -> Q14

So there is nothing to add: the threads are already on the tracker, and the new guide
(`docs/guides/cardinality.md`) deliberately does not present any of them as settled. Step 4's
substance is satisfied without writing to `plans/`, and the "do not touch plans/" constraint is
honored by leaving the file alone.

## The leftover that needs a later repoint

Deleting the migrated sections leaves two markdown links inside `plans/questions.md` pointing at
deleted `draft.md` headings, both to sections this migration removed:

- Q11 -> `../draft.md#the-core-idea-two-layers`
- Q32 -> `../draft.md#proposal-the-layer-shift-only-runs-one-way`

Repointing them (to the new guide, or to the questions themselves) is a one-line change each,
but the "do not touch plans/" hard constraint takes precedence over a broken-reference tidy-up,
so this migration leaves them as-is.

## Alternatives

1. **Leave `plans/questions.md` untouched (chosen).** Honors the hard constraint; the two links
   stay stale until a row allowed to touch the tracker repoints them.
2. **Repoint the two links to the new guide.** One-line fixes, followable references, but edits
   a file the hard constraint names.

## Decision

Took alternative 1. The four threads were already tracked, so step 4 needed no new questions;
the two stale `plans/questions.md` links are left for a tracker-maintenance row (or the
`draft-md-cleanup-review` cleanup row) to repoint.
