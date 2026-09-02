---
status: accepted
---

# Stream is the effect layer, typed

The fused `jsonlines(f(inputs))` loop made programs stream for real while the checker still
typed `inputs` as `Vec<T>` and `jsonlines(...)` as `Str`: whether a program streamed was a
backend-side pattern match, and a program one shape away from the pattern silently fell back to
materializing all of stdin. We decided `Stream<T>` enters the surface type grammar as the type
of effect-layer multiplicity: born only at sources (`inputs`, `lines`, and `range`, amended by
kantord/toylang#137), spellable in function signatures, consumed exactly once, dying only at
`collect` (reify spelled as a word) or at the top-level-only sink `jsonlines`, and never stored
in a record, a `Vec`, or another `Stream`. This settles draft.md's Q1 as "evaluation-level, but
typed": the silent streaming cliff becomes a checked property, and eager consumption of stdin
gets a visible spelling, `collect(inputs)`.

## Considered options

- First-class `Stream<T>` values, storable and nestable. Rejected: a held value of genuinely
  unknown extent is exactly what Q13's lean rules out, and it is the one irreversible option --
  once programs store streams, restricting back is a breaking change, while lifting a
  second-class restriction later is additive. `range` does not reopen this: it is a source, not
  a storable value, and `Int -> Stream<Int>` is a birth point the way `inputs` and `lines` are.
- Checker-internal streaming: surface signatures stay `Vec`-based and the checker classifies
  which functions are stream-safe. Rejected because a `Vec -> Vec` signature silently meaning
  two different things is the same implicitness the decision exists to remove.
- Status quo, the recognizer as a silent optimization. Rejected: the silent eager fallback is
  the defect, not an acceptable cost.
- For `jsonlines`: keep the `Str` result (a type asserting the whole output exists as one
  value -- a lie under streaming), or introduce an `Out`/unit type (answers Q35 as a side
  effect, without the argument that question deserves). Top-level-only sink keeps Q35 open.

## Amendment: range joins the sources (kantord/toylang#137)

`range(n)` was typed `Int -> Vec<Int>` when this ADR was first recorded, so the "born only at
sources" sentence named only stdin-backed birth points. The prelude-inventory survey (gh:105)
flagged that making `range` a Stream source reopens Q13's "no value-to-effect operator is
needed, because degrading a `Vec` forgets its extent and buys nothing" and, with it, Q1
(plans/prelude-inventory.md:56-66). gh:127 recorded the full background and the conservative
reading; the streams-and-sinks grill round ratified the change (2026-08-30), and gh:137 is the
build issue.

The ruling keeps Q13's substance while naming its boundary. What Q13 leans against is
degrading a value that already exists: turning a `Vec` back into a stream forgets the extent a
`Vec` promises and buys nothing. `range` degrades nothing -- `n` is one integer, and the stream
of values below it has no extent to forget, only one to generate. So `range` becomes a third
birth point, and the Euler `range | select | map` pipelines stream at constant memory instead
of materializing the whole `Vec`. It remains a source by every other rule: born in the
program's own body, single-use, dying at `collect` or `jsonlines`, never stored. A function
cannot conjure a stream from an argument it merely holds; `range` is a name, not a general
value-to-effect operator.

## Consequences

- The linearity and containment rules generalize machinery the monomorphic `Lines` type
  already had (single use, banned from records and Vecs, unprintable); `Lines` itself becomes
  `Stream<Str>`, and `range` joins the two as a third source.
- `length` stays `Vec`-only, keeping its no-fold promise; stream reducers are future work.
- `recognize_fusion`'s job shifts from guessing program shapes to reading types; an eager
  fallback for a stream-typed program becomes a compiler bug rather than a silent behavior.
- New rejection tests are required (nested `jsonlines`, twice-consumed stream, `Stream` in a
  record), since the output-equality corpus cannot observe any of this.
