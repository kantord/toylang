Board row `shell-out-build` (gh:158). Build the streaming pipe-to-subprocess primitive:
`lines | pipe_through("grep", [...]) | collect`.

Both open design questions are RULED:
- Output side (`stream-merge-tagged-design`): a fixed stdlib two-variant enum tags each
  relayed line by origin, e.g. `enum PipeLine { Stdout{text: Str}, Stderr{text: Str} }` --
  `pipe_through(cmd: Str, args: Vec<Str>) -> Stream<PipeLine>`. Verified: an ordinary
  closed-nominal enum already produces this shape with zero new type-system machinery.
- Input side (`stdin-stdout-splitting-design`): a symmetric `Stream<Str>` stdin argument --
  `pipe_through(cmd: Str, args: Vec<Str>, stdin: Stream<Str>) -> Stream<Line>`.

Context: `Stream<T>` today is source-only and single-consumer (`src/ty.rs:57-61`, `src/ast.rs:99`)
-- the only existing instance is `jsonlines`, which reads a stream FROM stdin/a file and writes
eagerly to the program's own stdout (see the per-backend `tl_jsonlines` helpers, e.g.
`src/emit_js.rs:202`, `src/emit_py.rs:218`, `src/emit_lua.rs:314`). Nothing today writes a
`Stream` value out to an external sink other than the program's own stdout -- feeding a
`Stream<Str>` to a spawned subprocess's stdin is new plumbing, needed for the stdin side of
`pipe_through` alongside the relay-in (subprocess stdout/stderr -> `Stream<PipeLine>`) side.

Scope this to ONE backend for the first commit (checkpoint-1 ruling, wizard submission
2026-09-07: one backend per commit, even a small multi-backend group recreates a smaller
version of the blast radius blamed for the original 7-backend-blob corruption). Pick whichever
backend has the most natural subprocess primitive to build against first (e.g. the native/Rust
backend, or a scripting backend with straightforward subprocess+pipe support) and say which one
you picked and why in your summary. Do not touch the other backends' emitters in this pass.

Concretely for the chosen backend:
1. Add the `PipeLine` enum (`Stdout{text: Str}` / `Stderr{text: Str}`) as a stdlib-predefined
   type, following however other builtin enums are declared (see the closed-nominal-enum
   mechanism used elsewhere in the checker/prelude).
2. Implement `pipe_through(cmd: Str, args: Vec<Str>, stdin: Stream<Str>) -> Stream<PipeLine>`:
   spawn the subprocess, feed it `stdin`'s lines, relay stdout/stderr lines back tagged by
   origin, streaming (not buffering the whole subprocess output before yielding).
3. A real example `.toy` program using `pipe_through` end to end (e.g. piping lines through
   `grep` or `cat`) that type-checks and produces correct tagged output when run.

Done-gate: the example program compiles and runs correctly for the chosen backend, both the
relay-out (stdout+stderr tagging) and relay-in (stdin feed) sides work, and `just check` passes
with no regressions on backends/programs untouched by this change.
