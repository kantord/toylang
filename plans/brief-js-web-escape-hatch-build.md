Board row `js-web-escape-hatch-build` (gh:160). Follow-up split from `js-node-web-split-build`
(archived, gh:160 ruling "one system, all four together"; that dispatch was scoped down to the
Node/Web split slice only, deferring the other three pieces -- this is one of them).

Context: the Web JS target (`JsTarget::Web` in `src/emit_js.rs:14-23`) already refuses any
program shape that would read stdin through node's `fs`, since a browser has no stdin --
see `reads_stdin` (`src/emit_js.rs:352-364`) and the hard error at `src/emit_js.rs:253-258`.
There is no equivalent problem for ordinary builtins that merely *implement* something
node-specifically today (e.g. `tl_collect_lines`, `src/emit_js.rs:130`, which reads stdin via
node's `fs` and is one of the refused shapes) -- there is currently no mechanism for a Web
build to supply a substitute implementation (e.g. reading from a browser API, a provided
string, or a fetch response) instead of hitting the compile-time refusal.

Build the escape hatch: a way for a toylang program or its build invocation to register a
different JS implementation for a builtin under the Web target, so a Web build can opt into
working code instead of the hard error. Concretely:
1. Survey which builtins in `src/emit_js.rs` currently hard-fail (or would need to) under
   `is_web()` and pick the mechanism shape (e.g. a CLI/build-config flag naming a JS module
   whose exports override specific helper functions by name, injected in place of the
   corresponding `*_HELPER` constant).
2. Implement it for at least `tl_collect_lines` end to end: a working example where a Web
   build supplies a substitute (e.g. reading from a JS string or a provided iterator) and the
   emitted program uses it instead of erroring.
3. Keep the Node target's default behavior unchanged -- this is additive only for Web.

Done-gate: a real example program using a stdin-reading builtin compiles for the Web target
via the new escape-hatch mechanism (previously a hard compile error) and its emitted JS
produces correct output when run with the substitute implementation supplied; `just check`
passes with no regressions on the Node target or on programs that don't use the escape hatch.
