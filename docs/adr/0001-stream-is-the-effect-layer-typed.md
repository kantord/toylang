---
status: accepted
---

# Stream is the effect layer, typed

The fused `jsonlines(f(inputs))` loop made programs stream for real while the checker still
typed `inputs` as `Vec<T>` and `jsonlines(...)` as `Str`: whether a program streamed was a
backend-side pattern match, and a program one shape away from the pattern silently fell back to
materializing all of stdin. We decided `Stream<T>` enters the surface type grammar as the type
of effect-layer multiplicity: born only at sources (`inputs`, `lines`), spellable in function
signatures, consumed exactly once, dying only at `collect` (reify spelled as a word) or at the
top-level-only sink `jsonlines`, and never stored in a record, a `Vec`, or another `Stream`.
This settles draft.md's Q1 as "evaluation-level, but typed": the silent streaming cliff becomes
a checked property, and eager consumption of stdin gets a visible spelling, `collect(inputs)`.

## Considered options

- First-class `Stream<T>` values, storable and nestable. Rejected: a held value of genuinely
  unknown extent is exactly what Q13's lean rules out, and it is the one irreversible option --
  once programs store streams, restricting back is a breaking change, while lifting a
  second-class restriction later is additive.
- Checker-internal streaming: surface signatures stay `Vec`-based and the checker classifies
  which functions are stream-safe. Rejected because a `Vec -> Vec` signature silently meaning
  two different things is the same implicitness the decision exists to remove.
- Status quo, the recognizer as a silent optimization. Rejected: the silent eager fallback is
  the defect, not an acceptable cost.
- For `jsonlines`: keep the `Str` result (a type asserting the whole output exists as one
  value -- a lie under streaming), or introduce an `Out`/unit type (answers Q35 as a side
  effect, without the argument that question deserves). Top-level-only sink keeps Q35 open.

## Consequences

- The linearity and containment rules generalize machinery the monomorphic `Lines` type
  already had (single use, banned from records and Vecs, unprintable); `Lines` itself becomes
  `Stream<Str>`.
- `length` stays `Vec`-only, keeping its no-fold promise; stream reducers are future work.
- `recognize_fusion`'s job shifts from guessing program shapes to reading types; an eager
  fallback for a stream-typed program becomes a compiler bug rather than a silent behavior.
- New rejection tests are required (nested `jsonlines`, twice-consumed stream, `Stream` in a
  record), since the output-equality corpus cannot observe any of this.
