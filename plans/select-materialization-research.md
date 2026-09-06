# select's representation: research needed before re-asking Q22

Round 2 of offload-boundary-design asked what `select` returns (Q22/Q14: a
masked view, a selection vector, or a materialized copy). The maintainer's
answer (docs/.grill/offload-boundary-design.round.yaml inbox capture,
2026-09-02) didn't pick A/B/C. It sketched a fourth shape none of the three
options covered, and asked for prior-art research before it gets re-asked.

## The maintainer's proposal

Same type as input (`Vec`), backed by a mask/view -- closest to option B's
promise (indexing stays live) but with a materialization trigger option B
didn't specify:

> select itself should not be automatically forcing a memory materialization,
> that should only happen when it gets a strong reference (i.e. no references
> to the pre-select input value remain)

This is the same "provably one reference" condition [[mutation-semantics-spike]]
already spiked for mutation-as-optimization (plans/mutation-semantics-spike.md)
-- here proposed as the trigger for turning a lazy `select` view into a real
buffer, not just for permitting in-place mutation.

Open questions the maintainer raised and did not resolve:

1. **What makes select's result indexable at all**, if it's not immediately
   materialized? A popcount-to-offset table, built lazily?
2. **Is `select`'s result actually a different type** -- a `Lens` or view type
   that does *not* get the indexing/promise guarantees of `Vec`, closer to
   option A after all, just triggered differently?
3. **Does the first index access materialize incrementally** -- i.e. does
   indexing element `k` materialize only up to `k`, so a second index request
   right after can reuse that partial work instead of paying for it twice?
   (Explicitly flagged as the part the maintainer is least sure about.)
4. **What do other languages do here** -- named as worth surveying before
   re-asking. Candidates: NumPy fancy-indexing vs. views (`arr[mask]` always
   copies; `arr[slice]` is always a view -- no lazy hybrid), Julia's `view`/
   `@views` (explicit, not automatic), Rust's `Cow<[T]>` (copy-on-write, but
   triggered by mutation intent, not reference count), pandas
   `SettingWithCopyWarning` (a real-world case where "is this a view or a
   copy" ambiguity caused enough user pain to need a runtime warning system).

## What to produce

A short survey answering (4), then a concrete proposal for (1)-(3) that a
future round can present as options with real code previews -- not the
original A/B/C, which this answer already moved past. Feed findings back into
offload-boundary-design (Q22) as a round 3 (or later) question, worded so the
"strong reference" trigger and the indexability mechanism are explicit parts
of each option, not left implicit.

## Status

Research only, nothing implemented. Filed as board row
`select-materialization-research` (gh: TBD).
