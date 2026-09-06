# Match<T> wrapper: what the experiment found before building it

Spike for board row `match-type-wrapper-experiment` (gh:122): experimentally build a
`Match<T>` wrapper where `.` entering a `|` chain wraps the scrutinee, traits like `%`
get generic impls over `Match<T>` per underlying `T` applying only past a passing
boolean guard, and an arm's aliases become named fields on a result struct. The ruling
(gh:122) asked to see what goes wrong before ratifying. This note records what tracing
the three load-bearing changes against the existing compiler turned up, because that
trace is where the failure modes already are.

## The wrapping point is real and localized

`. ` entering a `|` chain gets its subject in exactly one place: `pipe` (`src/check/mod.rs:1330`)
binds the left side to a fresh local and builds the inner context with
`ctx.with(Some((value.ty.clone(), local)))`. `match_chain` (`src/check/mod.rs:1619`)
and the `MatchCall` path reach the same subject mechanism. So the first pillar of the
design -- wrapping the scrutinee -- is a one-line change at one site, not a
multi-file spread. That part of the verdict holds under the code.

But there is a second wrapping decision the design does not name: a guard arm's body
keeps `.` as the *enum* subject (`src/check/mod.rs:1646`), and a `bool_or_in_arm_body`
style program does `[1, 5] | map(. > 3 -> (. == 5 or . == 6) or . == 1)`, comparing
`.` inside a guard body. Wrapping the guard-arm subject as `Match<T>` therefore changes
the type every existing guard body sees. A wrapper that is not transparent -- one that
does not delegate comparison, field access, `select`, and the cardinality helpers to
its inner `T` -- breaks these programs. The spike's first concrete finding: "`.` enters
a `|` chain" is not the same as "`.` inside a guarded arm is the wrapped value," and
deciding which one is the wrap point is a design choice the brief leaves open.

## Trait dispatch is dead, so "generic impls over Match<T>" has no machinery to hook

`TraitDecl` and `ImplDecl` are parsed (`src/ast.rs:214`, `src/ast.rs:238`) into
`File.traits` and `File.impls`, and nothing reads them. `check_module`
(`src/check/mod.rs:518`) consumes only `module.enums` and `module.defs`; a grep for
`.traits` / `.impls` outside `parse.rs` and `ast.rs` returns nothing. The checker's
polymorphism today is three closed mechanisms (`plans/trait-interface-research.md`
catalogues them): the per-operator `binary`/`plus` match (`src/check/mod.rs:3095`,
`src/check/mod.rs:3200`), the hand-special-cased polymorphic builtins in `call`, and
single-signature function dispatch.

The design's second pillar -- `%` getting generic impls over `Match<T>` -- therefore
cannot hook into any existing table. The honest options are: (a) special-case `%` (and
whatever else) over `Match<T>` in `binary`, one operator at a time, which is what a
spike would actually do to find out whether general dispatch is needed; or (b) build
real trait dispatch -- `Self` substitution, impl lookup by `(trait, type)`, overlap and
coherence -- which is an architecture change, not a spike. Wiring (b) in now would be
premature: nothing in the corpus or the design yet demands it, and it is the
hard-to-reverse decision the brief exists to defer.

## The guard-gating has no check-time meaning

"applying only past a passing boolean guard" reads as: an operator over a `Match<T>`
applies only when the arm's guard passed. But a guard is a runtime `Bool` the checker
cannot see through (`src/check/mod.rs:1704` says so in so many words: a guard is a
runtime value, and guards do not count toward exhaustiveness). So "apply `%` to
`Match<T>` only past a passing guard" is undecidable at check time. Either the operator
is checked unconditionally over `Match<T>` and the runtime guard decides whether the
arm runs (which makes the "past a passing guard" clause empty at type-check time), or
the design is really about a different shape -- e.g. the result struct, below, carrying
only the matched fields. This is the clearest conceptual failure mode the spike exists
to surface, and it is in the design text, not in the code.

## The result struct is new work threaded through every backend

`tir::MatchArm` (`src/tir.rs:197`) carries `variant` / `guard` / `payload` / `body`;
there is no alias field. `Kind::Match`'s result is the common body type. Turning the
match's output into a record whose named fields are the arms' aliases is new TIR, new
checker logic, and a change to the per-backend `Kind::Match` emission in all eight
emitters (`emit_js.rs`, `emit_py.rs`, `emit_lua.rs`, `emit_go.rs`, `emit_rs.rs`,
`emit_jq.rs`, `emit_llvm.rs`, `emit_toylang.rs`). Nothing about that is a refactor of
existing code; it is the third pillar's own new surface.

## Why no refactor-first

The one place the existing code is genuinely repetitive is the per-backend `Kind::Match`
emission: the four-case `match (&arm.variant, &arm.guard)` over PayloadVariant /
PlainVariant / Guard / Default and the `if *partial || i+1 < arms.len()` test-wrap
predicate repeat across JS, Lua, Py, jq, and part of Go. Extracting a shared `arm_test`
classifier and `arm_needs_test` predicate is line-neutral at best, and the `needs_test`
half bakes in the first-match-wins if/else shape that a spike centered on "apply past a
passing boolean guard" may deliberately move away from. Extracting it now would be
premature -- the experiment is the thing that would tell us whether that shape survives.
`match_chain` is 110 lines with interleaved phases, but splitting it is a reshuffle, and
the experiment threads alias-collection through the per-arm loop either way.

## What this means for the done-gate

The spike's real findings are not code-shaped. The wrapping point is localized and
cheap; the "generic impls over Match<T>" pillar has no trait machinery to hook and
should be a per-operator special case until general dispatch is actually needed; the
guard-gating clause is not check-time decidable; and the result struct is a clean,
large new surface across all eight backends. Building the third pillar is a real
multi-backend change that deserves its own implementation and corpus cases, not a
refactor of the existing match code -- which is the shape the original task's
"before ratifying" language points at. The experiment's value is that it says where
the work actually is before anyone writes it.
