# Crate simplification survey

Issue #13. Inventoried the compiler and harness code (`src/`, `tests/`, `main.rs`) for hand-rolled
subsystems that a well-established crate could genuinely simplify. [ADR 0004](../docs/adr/0004-backends-emit-source-run-by-real-toolchains.md)
bounds the scope: a crate can serve the compiler or the test harness, never an emitted program or
its runtime. `winnow`, `insta`, `serde`/`serde_json`/`serde_norway`, `inkwell`, `mlua`, and
`tempfile` are already in and out of scope here; the question is what else, if anything, earns a
place next to them.

One recommendation to install, one explicit warning against a change that looks obvious but
isn't, and several "already served" or "not worth it" findings recorded so the next survey
doesn't re-walk the same ground.

## Recommend: `anyhow` for the `Box<dyn std::error::Error>` boilerplate

`Box<dyn std::error::Error>` is the return-error type on 10 functions across `src/lib.rs` and
`src/main.rs` (`run_on`, `link`, `link_rust`, `run_subprocess` and its five callers, `main`'s
`run`/`build`). Eight of those sites build the error by hand: `format!(...).into()` or
`.map_err(|e| format!("could not run `{label}`: {e}"))?` (`src/lib.rs:142,167,280,282,305,335,344,350`).
This is the standard shape `anyhow` exists for: a leaf binary/library boundary that wants one
error type to converge on, string context attached at each hop, no need to match on error
variants downstream.

Concretely: `Result<T, Box<dyn std::error::Error>>` becomes `anyhow::Result<T>`, and
`.map_err(|e| format!("could not run `{label}`: {e}"))?` becomes
`.with_context(|| format!("could not run `{label}`"))?`. `error::Error` (this crate's own
parse/check error type) already implements `std::error::Error`, so it keeps working with `?`
unchanged; nothing about the `Error`/`Span` type moves.

**What could break**: `anyhow::Error` doesn't implement `Clone` or `PartialEq`, and downcasting
is needed to recover a specific error variant. Nothing today matches on these errors for control
flow -- they only ever get displayed (`main.rs`'s `eprintln!("toylang: {path}: {e}")`) or
snapshotted via `.to_string()` (`tests/unwrap.rs:88-91`). `anyhow::Error`'s `Display` prints only
the top-level message by default, same as the `String`/`Box<dyn Error>` it replaces, so existing
snapshots should be unaffected unless new `.context()` calls are added on top of an existing
message, which would need a snapshot re-record (`cargo insta review`). If a future change wants
distinct exit codes per failure kind (e.g. "cc missing" vs "cc failed" in `link`), `anyhow`
works against that -- it would need typed errors (`thiserror`) instead, or downcasting. Nothing
in this repo does that today.

**Migration cost**: mechanical, roughly 10 signatures and 8 call sites, one sitting.

## Investigated, not recommending: consolidating the per-backend string escaper

Six of the seven backends hand-write their own string-literal escaper, converting a toylang
string into that target's own source syntax: `emit_go.rs:956-964`, `emit_jq.rs:463-471`,
`emit_js.rs:572-580`, `emit_lua.rs:650-658`, `emit_py.rs:479-487`, `emit_rs.rs:1099-1112`
(`rs_string`). Six near-identical functions is exactly the shape that looks like an obvious
extraction, crate or shared helper.

It isn't, for the same reason a shared printer would be wrong (issue ground rule 2, ADR 0002
"backends as falsifiers"): each function encodes what *that target language's string literal
grammar* accepts, not JSON's. They agree on `\"`, `\\`, `\n`, `\r`, `\t` because those five
happen to be common across Go, jq, JS, Lua, Python and Rust -- but that agreement is the thing
under test, not an assumption to bake into shared code. A language whose escaping actually
differs (a target needing `\v` or rejecting an escape the others accept) would round the
difference away silently if the six backends shared one function, the same failure mode ADR
0002 calls out for the printers overall. This is corpus territory, not crate territory: if the
escapers are in fact identical today, the corpus should be proving that by running all six
against a string with the interesting characters, not by deduplicating the code that could catch
them diverging.

## Not a candidate at all: the hand-rolled JSON parsers inside the Rust and native backends

`emit_rs.rs` embeds a JSON parser (`TlParser`, `PARSER_HELPER` starting at `emit_rs.rs:130`) and a
`tl_quote` escaper (`QUOTE_HELPER` starting at `emit_rs.rs:358`) as Rust source text -- these are not
compiler code, they are string constants the compiler writes out to *become part of the emitted
program*. `runtime/toylang.c`, linked into every native/LLVM binary, carries the same thing in C
(`tl_quote` at `runtime/toylang.c:186`, the `tl_json` parser from `runtime/toylang.c:243`
onward). JS and Python don't need this (`JSON.parse`/`JSON.stringify`, `json.loads` -- checked at
`emit_js.rs:155,214,249` and `emit_py.rs:151,155,180`); Rust has no JSON in its standard library
and the native backend has no standard library at all, so both write their own.

This is ADR 0004's ground rule 1 in its most literal form, not a close call: `link_rust`
(`src/lib.rs:293-313`) invokes `rustc` directly on one file with no Cargo project, so there is no
dependency resolution available to the emitted program even if a crate were installed in this
repo's own `Cargo.toml`. `serde_json` in *this* crate's dependencies has no bearing on what code
`rustc` can see when it compiles the string `emit_rs.rs` produced. The only way to give the
emitted Rust program a JSON crate would be generating a `Cargo.toml` alongside it and shelling
out to `cargo build` instead of `rustc` directly -- a real architecture change ADR 0004 already
rules out, not a dependency swap.

## Investigated, not recommending: `clap` for `main.rs`

`main.rs` is 90 lines with three subcommands (`run`, `emit`, `build`) and one optional backend
name. The whole parse is one `match args.as_slice()` (`main.rs:11-25`). `clap`'s derive API would
replace that with an attribute-annotated struct, a proc-macro dependency, and a longer `--help`
than the language currently needs -- more surface for less code than it looks, for a CLI this
small. Revisit if the command surface grows past a handful of flags.

## Investigated, not recommending: a subprocess crate (`duct` et al.) for `lib.rs`

`run_subprocess` (`src/lib.rs:324-353`) is already the single chokepoint every OS-process backend
(`node`, `python3`, `go run`, `jq`, the linked native/Rust binaries) goes through; per-backend
code only builds the `Command` and its args. What `duct` and similar crates are for --
collapsing a `Command`/`spawn`/`wait`/pipe dance into one call -- is mostly done here already.
The one thing that doesn't fold into that shape is `Feed::Live` vs `Feed::Text`
(`src/lib.rs:234-257`): live runs inherit stdio straight through with no capture, piped runs
buffer for the test harness to read back, and that distinction is the point of ADR 0004's
streaming architecture, not incidental plumbing a pipeline-builder crate would remove. Adopting
one would trade ~30 lines of code that already says exactly what it does for a dependency that
doesn't model the live/captured split any better.

## Already served: JSON handling on the harness side

`serde`, `serde_json`, and `serde_norway` already cover every JSON/YAML touch point: corpus case
loading (`tests/support/mod.rs`, `#[derive(Deserialize)] #[serde(deny_unknown_fields)]`), input
validation against the language's own `Type` (`src/input.rs`, which necessarily hand-walks
`Type` since it's validating against a dynamic type description `serde`'s derive has no way to
see, not a static Rust struct), and the site export (`tests/export_site.rs`, built with
`serde_json::json!`). No gap here; the pattern already matches "few, well-chosen dependencies."

## Already served: test scaffolding

`tests/support/mod.rs` and `tests/corpus.rs` (the agreement harness) are hand-rolled but thin:
one `Deserialize` struct with `deny_unknown_fields` catching the exact bug the header comment
describes (a misspelled key silently becoming a case that asks for nothing), and one loop over
`Backend::ALL` that collects every failure before asserting, on purpose -- so one run shows every
backend/case combination that broke, not just the first. A table-testing crate (`rstest` and
similar) would turn this into N separate `#[test]` functions and lose that aggregate report,
which is the actual value of the current shape. Not a gap; the hand-rolled loop is doing
something a parametrized-test crate doesn't.

## Not yet, flagged for later: span-rendering for diagnostics

`src/error.rs` carries a `Span` on every error but only prints `(at byte N)`
(`src/error.rs:17-21`) -- the doc comment on `Error` says plainly that "nothing renders them
against the source yet." Crates like `codespan-reporting` or `ariadne` exist for exactly this
(printing a caret under the offending byte range with the surrounding source line), but
recommending one now would be speculative: there's no rendering code to replace, and installing
a diagnostics crate ahead of the feature it serves is the inversion this survey is supposed to
avoid. Worth revisiting once source-rendered errors are actually being built.

## Investigated, ruled out: `assert_cmd` for `tests/streaming.rs`

`tests/streaming.rs` spawns the compiled `toylang` binary and asserts a record shows up on stdout
*before* stdin is closed (`assert_streams_first_record`, `tests/streaming.rs:51-73`), using a
background thread and `mpsc::recv_timeout` to make the race explicit. `assert_cmd` is built for
run-to-completion assertions (spawn, wait, check the final output); it has no facility for
asserting on output that arrives mid-run while stdin stays open, which is the entire point of
this file. No crate here does what the hand-rolled version does.
