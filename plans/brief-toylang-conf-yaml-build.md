Board row `toylang-conf-yaml-build` (gh:160). Add the `target: node | web` selector to
`toylang.conf.yaml` and wire it into the compiler so the JS backend actually picks a target
instead of always emitting for Node.

Background: `src/config.rs` already exists (landed by a prior sandbox attempt on the sibling
row `js-web-escape-hatch-build`) and defines `Config { web: Web }`, where `Web` holds the
Node/Web escape-hatch replacements (`input`, `lines`, `read_line`). `Config::load()` walks
upward for `toylang.conf.yaml` and parses it. None of this is wired to anything yet: `lib.rs`
only declares `pub mod config;`, and `src/emit_js.rs::JsTarget` is hardcoded to `JsTarget::Node`
at every call site (`src/main.rs:140`, `src/lib.rs:77`, `src/lib.rs:233`). This row's job is the
missing wiring, not re-deriving the `Web` escape-hatch struct.

`js-web-escape-hatch-build` itself is still open (separate board row, currently blocked on a
maintainer question) -- do not try to finish its scope here. Land `Config.web` plumbing only as
far as `emit_with`'s existing `web: &Web` parameter already accepts (check `src/emit_js.rs:251`
`emit_with(program, target, web)`); if wiring the escape hatch through cleanly requires design
decisions belonging to that other row, leave it as a `Web::default()` (today's implicit
behavior) and say so in your summary rather than guessing.

Concretely:
1. Add `target: JsTarget` (or an equivalent `node`/`web` enum) to `Config` in `src/config.rs`,
   defaulting to `Node` when absent so existing (target-less) `toylang.conf.yaml` files and the
   no-file case keep today's behavior exactly.
2. At the JS emission call sites (`src/main.rs:140`, `src/lib.rs:77`, `src/lib.rs:233`), load
   the config (`Config::load()`) and pass its `target` (and `web`, via `emit_with`) through
   instead of the hardcoded `JsTarget::Node` / implicit `Web::default()`. `Config::load()`
   already returns `Result`; propagate its error the same way other config/IO errors are
   surfaced at these call sites.
3. A real `.toy` program plus a `toylang.conf.yaml` with `target: web` next to it that changes
   the emitted JS backend's target, verified either by a compiler test or a documented manual
   run -- say which in your summary.
4. Do not touch the escape-hatch semantics (`web_stdin_refusal`, the `Web` field wiring inside
   `emit_js.rs`'s stdin-reading branches) beyond passing the already-loaded `Web` value through;
   that refinement is `js-web-escape-hatch-build`'s scope.

Done-gate: `toylang.conf.yaml`'s `target` field actually changes which `JsTarget` the compiler
emits for, a target-less or missing config file compiles identically to before this change, and
`just check` passes with no regressions.
