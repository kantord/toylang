Board row `vec-as-dataframe-type-research`: research spike, not a design ruling and not a
build of the feature itself. Write findings to `plans/vec-as-dataframe-type-research.md`.

Spun off from `dense-tensor-type` round 5 (maintainer freeText, 2026-09-02): the maintainer's
lean is not a new flat-buffer tensor value kind, but treating `Vec` itself as inherently the
dataframe/tensor-capable type -- a bare-minimum set of dataframe primitives (reshape,
transpose, broadcast, reduce) implemented over the existing simple `Vec<Vec<Num>>` layout.

Investigate, with real code where possible:
1. Whether "Vec is the dataframe type" needs an explicit type-design change (formally making
   Vec a multi-dimensional type in the checker/type system), or can be built as ordinary
   stdlib primitives over the current simple `Vec<Vec<Num>>` type, unchanged.
2. Run both approaches against the sensor-pipeline example already worked out in
   `plans/dense-tensor-design-research.md` -- concrete code, not just prose comparison.
3. Note the transpose sub-question flagged in the `dense-tensor-type` row: single
   two-dimensional transpose vs. a fully generalized multi-dimensional transpose, and what
   each choice would cost under both approaches above.

This is groundwork for a future re-ask on `dense-tensor-type`, not the ruling itself -- do not
pick a final answer, just ground the tradeoffs in working code and hand back findings.
Done-gate: `plans/vec-as-dataframe-type-research.md` exists with both approaches demonstrated
against the sensor-pipeline example, `just check` still passes (no regressions from any
throwaway repro code left behind -- clean those up before finishing).
