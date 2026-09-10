Board row `dense-tensor-type-build` (gh:175). Builds the ruling from board row `dense-tensor-type`
(wizard submission, dense-tensor-construction-iteration-nulls round, 2026-09-08; see
`plans/questions.md` Q17/Q18/Q19 and `plans/dense-tensor-design-research.md` /
`plans/vec-as-dataframe-type-research.md` for the full research trail).

Ruling, all three parts:
1. **No separate `Tensor` value kind.** `Vec` itself is the tensor/dataframe-capable type --
   rectangular (`Vec<Vec<T>>` today, since there is no first-class multi-dim buffer), shape-checked
   at construction. Do not introduce a new `Type::Tensor` variant; build this as a builtin over the
   existing `Vec` type in `src/ty.rs`.
2. **Construction: `tensor(n; m)`, one stage, no width commitment.** `Float` does not exist yet
   (only `Int`/`Int64`, see `src/ty.rs`), so do not gate this on a number width the way the
   discarded `@f32 | reshape(n; m)` sketch did. `tensor(n; m)` takes a flat or nested input and
   both narrows (hard-fails on a non-numeric, ragged, or null element -- see part 3) and shapes
   into `n` rows of `m` elements each, in one call, one failure point.
3. **Nulls: hard-fail only, no bitmask.** Construction refuses outright if any element is null,
   ragged, or the wrong type -- same posture `contains_vec`-gated operator refusals already use
   in `src/check/mod.rs` (Q2 guard) and `src/ty.rs:246`. Caller must resolve gaps (drop/fill)
   before calling `tensor`. Do not implement an Arrow-style validity bitmask; this reverses an
   earlier design leaning and the maintainer flagged the pick as tentative -- if this build finds
   a real case that needs masked nulls through construction, stop and note it rather than
   building a bitmask speculatively.
4. **Transpose/column-access view, built now.** `.[]` on a rank-2 tensor already yields rows (it's
   just existing `Vec<Vec<T>>` iteration, Q18, no new work needed there). What's missing is a
   `transpose` builtin/view for column-wise access, e.g. `.counts | transpose | map(sum(.))` for
   per-column reductions. Follow the existing `Builtin` enum pattern (`src/tir.rs`, tagged in
   `src/tags.rs`, implemented per-backend in `src/emit_*.rs` the same way `Sum`/`Max` are -- see
   `Builtin::Sum`/`Builtin::Max` call sites across `emit_go.rs`, `emit_js.rs`, `emit_py.rs`,
   `emit_lua.rs`, `emit_rs.rs`, `emit_jq.rs`, `emit_llvm.rs` for the seven-backend fan-out shape a
   new builtin needs).

Scope: implement `tensor(n; m)` and `transpose` as new builtins, hard-fail semantics on
construction, across all seven backends (the corpus harness requires every backend to agree on a
program's output, or all refuse it together). Do not build a new number type, a validity bitmask,
or a first-class multi-dimensional type -- all three are explicitly out of scope per the ruling
above.

Done-gate: a real program constructs a tensor via `tensor(n; m)` from row-major input, computes a
per-row reduction (`map(sum(.))` or `map(fold(add; 0))`) and a per-column reduction via
`transpose | map(sum(.))`, and gets the correct answer identically across all seven backends
(the existing corpus-conformance harness is the check). A program that calls `tensor(n; m)` with a
null, ragged, or wrong-typed element is refused (compile-time or hard runtime failure, matching
how `contains_vec`-gated refusals already behave) on every backend, not just some.
