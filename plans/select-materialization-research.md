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

Research complete. The survey and a concrete proposal for questions 1-3
are appended below, and the re-ask round exists as an ephemeral grill file
(`docs/.grill/select-materialization.round.yaml`, gitignored, deleted on capture).
Nothing implemented in src/. Filed as board row
`select-materialization-research` (gh:176).

## Survey: what the four surveyed languages actually do

Facts verified against current docs: NumPy 2.5 indexing, Julia array
docs, Rust `std::borrow::Cow`, and pandas 3.0 (its copy-on-write page, and
the indexing page's chained-assignment note. This answers open question (4) and
gives the re-ask's options their prior art.

**NumPy: mask selection copies eagerly, slice selection views.** Boolean
and integer array indexing is "advanced indexing", and the docs state it flatly:

"advanced indexing always returns a copy of the data (contrast with basic
slicing, which returns a view)". `arr[mask]` is defined via compaction: the docs
call a boolean index "practically identical to `x[obj.nonzero()]`", i.e. mask
selection IS selection-vector selection, with an eager copy. Basic slicing is
always a view with no compaction. There is no lazy hybrid for masks: mask
selection either copies eagerly, or stays masked via the opt-in `MaskedArray` type,
with a parallel mask array. Views pin their base: the docs warn that "the small
portion extracted contains a reference to the large original array whose memory
will not be released until all arrays derived from it are garbage-collected" -- the
same pinning cost the draft already names ("either view pins its whole source buffer
alive", questions.md Q14).

**Julia: views are explicit, opt-in, and a distinct type.** `x[1:10]`
copies by default;`@view x[1:10]` or `view(x, 1:10)` return a `SubArray`,
a subtype of `AbstractArray`, that "lazily references the parent" and indexes by
"computing the indices to access or modify the parent array on the fly". `@views`
flips a whole block to view semantics. Nothing is automatic. Views are a
distinct type from `Array`, but generic code writes against `AbstractArray`, so most
consuming functions accept either. The docs' views select by index arrays and
ranges, not by masks: a mask has no parent-index mapping to recompute, which is
also why NumPy defines mask indexing via `nonzero()` instead of as a view.

**Rust `Cow<[T]>`: one type, two states, clone-on-write on mutation intent.**
`Cow` is "a smart pointer providing clone-on-write functionality": an enum with
`Borrowed(&'a B)` and `Owned(<B as ToOwned>::Owned)`. `to_mut()` "clones the
data if it is not already owned". No reference counting anywhere: the docs defer
refcounted clone-on-write to `Rc::make_mut`/`Arc::make_mut`, which is what `Cow`
itself is not. The trigger is mutation intent (an explicit call), not alias
count. For select, the lesson is the shape:`Cow` keeps the type uniform (one type,
two states) and moves the clone to the moment a mutation demands an owned buffer.

**pandas: the ambiguity was real, and the resolution was to abolish it.**
`SettingWithCopyWarning` existed because the view-vs-copy question had no one-rule
answer: whether `df[...]` returned a view or a copy depended on the operation, and
the warning fired at the *write*, at runtime, from an internal heuristic. That is
the failure mode to avoid: a distinction not decidable from the source, surfaced
only by a runtime warning. pandas 3.0's answer is copy-on-write as the default
and only mode: one deterministic rule ("any DataFrame or Series derived from
another in any way always behaves as a copy"), chained assignment"consistently
never work"(raised as `ChainedAssignmentError`, not silent), and copies deferred
until a write. Two details bear directly on select: first, the deferral is bounded
by reference liveness --the docs' "reassign the result to the same variable: no
copy is necessary" note, and"creating multiple references keeps unnecessary
references alive and thus hurts performance" -- which is the maintainer's
strong-reference condition wearing pandas clothes; second, the trigger is a *write*,
and toylang has no writes, so the trigger has to be translated -- which is what
the strong-reference condition does.

## What the survey settles for Q1-Q3

**Q1 (what makes a lazy select indexable): a selection vector or a
mask+popcount table, whichever the layout makes cheaper.** A mask does not
have a Julia-style free view: there is no parent-index mapping to recompute, which
is exactly why NumPy defines mask selection via `nonzero()`. The two mechanisms are
dual representations of the same compaction: the selection vector of surviving
indices (O(1) per access, memory O(survivors))and the mask plus a
popcount-to-offset table (O(1) per access after an O(n) lazy build that touches mask
bits only, never element data. For struct-of-arrays, the mask+popcount side is the
cheaper one: it touches only the mask column, and"compaction is the only part that
touches element data at all"(draft.md:1280) is the observation that makes a masked
view cheaper than a copy there. Either way, indexing is O(1) per access after one
O(n) build, and no element data moves at select time.

**Q2 (is the result a different type: no.** The same-type shape is the cheap
one in every surveyed language that has it:NumPy keeps the type uniform by copying
eagerly (and paying the allocation on every mask select); Rust `Cow` keeps the
type uniform by hiding the state in an enum (and pays only when mutation demands an
owned buffer); pandas keeps the type uniform with CoW. Julia's view is a distinct
type, but only because generic code writes over `AbstractArray` -- toylang's
concrete monomorphic types would put the view/copy split into every consuming op's
signature instead. The distinct-type route also contradicts the existing ruling that
vectorizability is silent, and not type-visible("silent/cardinality-derived
vectorizability CONFIRMED (no type-level effect)", the offload-boundary-design
round-2 answer); the dense-vs-masked distinction is the same kind of thing: an
implementation state with different launch preconditions, not a type-level effect.
So Q22, restated, answers itself: not distinguishable in the type, distinguishable
only where it matters (backend scheduling).

**Q3 (incremental materialization: no.** No surveyed language materializes
incrementally:NumPy copies eagerly; Julia never compacts until an explicit `collect` (and has no mask views at all);`Cow` clones the whole buffer on first `to_mut()`; pandas CoW's deferred copy is all-or-nothing on first write. And
under either Q1 mechanism, incremental compaction buys nothing: any-k indexing is
already O(1) (popcount table or selection vector), sequential ascending access is
already a running offset, and a third state (masked, prefix-dense, dense) complicates
the strong-reference trigger for a case no surveyed language found worth serving. The
honest trade the round should name: indexing once pays an O(n) build over mask bits (cheap, no element data movement) instead of today's O(n) element copy
at select time, repeated random indexing is O(1) each after that, and the view
pins its source until materialization.

## Proposal for the re-ask(Q22)

The maintainer's fourth shape is the survey's consensus translated into a pure
pipeline: (a) same type as input(`Vec`), mask/selection-vector backing as an
implementation detail;(b) indexable via the lazily built selection/popcount machinery(Q1);(c) materialization only on strong reference --a second use of the
result, an escape into a Vec-typed context, or a dense-demanding op (sort,
concat...); with the compaction allowed to reuse the input's storage when the input has
no other reference (the provably-one-reference condition from
[mutation-semantics-spike.md](mutation-semantics-spike.md), now as a materialization
trigger rather than a mutation permission; the pandas "reassign to the same variable: no
copy" note is the same idea in another language. Q3's incremental idea drops: the machinery that makes indexing O(1) already makes it redundant.

The re-ask should present three options, each stating its indexability mechanism and materialization trigger explicitly, rather than the original A/B/C:

**A. Eager compaction(NumPy fancy indexing.** `select` compacts at select
time into a fresh dense `Vec`. Indexability: plain `Vec` indexing, no extra
machinery. Trigger: none -- nothing is lazy, and strong references never matter. Cost: the allocation the maintainer's answer wanted to avoid, paid on every select even
when the result is consumed once. This is toylang today (draft.md:1279:"`select`
is a copy today").

**B. Same type, mask/selection-backed, compact on strong reference (the
maintainer's proposal, worked out.** `select` returns `Vec`; behind the name a
{source, mask-or-selection} pair with no eager compaction. Indexability: a
selection vector (or mask+popcount table), built lazily on first index, O(1) per
access after an O(n) build over mask bits. Trigger: compaction into a real buffer
at the first strong reference; a second use of the result, an escape into a Vec-typed
context, or a dense-demanding op; in place into the input's storage iff the input has
no other reference, else a fresh copy. No incremental materialization. Surface
syntax: unchanged --this option is invisible in toylang code, and that is its
point. Cost: new backend machinery, anda view pins its source until materialization.

**C. Distinct view type (Julia `SubArray` / NumPy `MaskedArray`.** `select`
returns a new type (spelling for the round to pick, `Masked<T>` or similar), distinct
from `Vec`, indexable via the same machinery, and elementwise-pipeable, but without
Vec's dense promises. Materialization into a `Vec` is explicit(`collect(...)`,
spelling proposed) or implicit at any Vec-typed boundary (a function param typed
`Vec`, a concat/sort operand...). Trigger: any escape into a Vec context forces it,
and the view/copy split is visible in the type. Cost: two Vec-like types, every
Vec-typed function becomes a conversion site or a `Masked` overload, unless the
language grows `AbstractArray`-style generics it does not have.

The pandas `SettingWithCopyWarning` era is the "don't" every option must avoid:
whatever the view/copy rule is, it must be decidable from the source, not detected at
runtime.

## What would settle each

A versus B: whether an allocation per select is acceptable when the pipeline is
memory-bound --a select-heavy benchmark would measure it; andwhether the pinning
cost (B's views pin their sources) shows up in real pipelines. B versus C: whether
any realistic toylang program would want to observe the type distinction itself (accept a masked view as such); if none, C is machinery with no customer.



Derived: the survey facts from the cited docs (NumPy 2.5 indexing docs, Julia array docs,`std::borrow::Cow` docs, pandas 3.0 copy-on-write docs); the
strong-reference tie from [plans/mutation-semantics-spike.md](mutation-semantics-spike.md); the vectorizability-silent ruling from plans/board.yaml's offload-boundary-design round-2 answer. Agent-invented: the Q1-Q3 answers, and the three-option shape for the re-ask (the B option's mechanism sketch most of all).