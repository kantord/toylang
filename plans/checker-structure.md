# Checker structure survey

Issue #32, feeding [`check-rs-split-decision`](board.yaml). `src/check.rs` is 2049 lines today
(`wc -l`, 2026-08-28), already the largest file in the crate and one of the three files
`max_file_lines = 1000` was set to name (`.claude/checks/limits.toml`'s comment lists
`emit_llvm.rs`, `check.rs`, `emit_rs.rs`). It has grown since the split decision was filed: the
board item recorded 1874 lines (`plans/board.yaml:293`), so 175 more have landed without the
[type-flow rework](type-flow.md) even starting (`status: todo`, `plans/board.yaml:187`). That
rework is not a one-off addition; it grows exactly the part of the file most under discussion
here, so any split has to survive the file's current trajectory, not just its current size.

## How this checker is already structured

Four things share one file today, and they are not evenly coupled to each other.

**Type resolution.** `TypeEnv`, `enum_map`, `alias_map`, `resolve_enum`, `signatures`, and
`resolve` (`src/check.rs:548-784`, ~237 lines) turn the surface syntax of types — `TypeExpr`,
`EnumDecl` — into `ty::Type`. It runs once, eagerly, before any expression is checked (`enums`
and `aliases` resolved at `src/check.rs:71-102`, called from nowhere else), and touches neither
`Expr` nor `Tir`. Nothing downstream calls back into it except `resolve` itself (recursively) and
`construct`/`sole_owner` reading the already-resolved maps.

**The bidirectional expression engine.** `synth` (`src/check.rs:1017-1717`, the one large match
over every `Expr` variant), `expect` (`src/check.rs:1952-2035`), `access` (`src/check.rs:1755-
1855`), `binary` (`src/check.rs:1857-1948`), plus the helpers each of them shares state with
through `Ctx` (`src/check.rs:9-68`): `construct`, `collect`, `mapper_ctx`, `rebase`, `sole_owner`.
This is also where `Expr` becomes `tir::Tir` — checking and lowering are the same walk, on
purpose (see [pass ordering](#pass-ordering-and-the-fused-check-lower-pass) below).

**Stream linearity.** `StreamBinding`, `LinearViolation`, `stream_uses`, `check_linear`
(`src/check.rs:254-394`, ~141 lines) is a separate, later pass: it walks a `Tir` that already
exists, counting how many times a stream-typed binding is consumed along every path, and it never
touches `Expr` or `Type` resolution at all.

**Dead-code pruning.** `prune_unreachable`, `calls_in` (`src/check.rs:396-488`, ~93 lines) is
smaller and just as separate: it walks the finished `Tir` a third time to compute which functions
the program body can reach, for the backends' and `tags::node_types`'s benefit. It has nothing to
do with typing.

`check` itself (`src/check.rs:70-224`) is the driver that sequences these four in order and adds
the cross-cutting checks that only make sense once everything else is known (`input`/`inputs`/
`lines` mutual exclusion, `src/check.rs:193-216`).

## How real compilers structure this layer

### rustc's query system solves a different problem than this file has

rustc organizes its entire middle layer as memoized, on-demand queries (`K -> V` functions with a
dependency graph rustc tracks for incremental reuse across compiler invocations) rather than as a
fixed sequence of passes
([rustc-dev-guide, incremental compilation](https://rustc-dev-guide.rust-lang.org/queries/incremental-compilation.html)).
That architecture earns its complexity by avoiding recomputation across edit-rebuild cycles on a
large, multi-crate build graph — the same problem an LSP has, only sharper. Nothing in this repo
has that shape: `toylang run`/`emit`/`build` (`src/main.rs`) is a one-shot process over one file,
with no watch mode, no daemon, and per [ADR 0004](../docs/adr/0004-backends-emit-source-run-by-real-toolchains.md)
the compiler is never even a *runtime* dependency of what it produces, let alone a long-lived
process reusing state across runs. A query system would be solving a problem toylang does not
have, at the cost of restructuring every function here into trait-based query providers. This
axis does not apply, independent of the crate question below.

### The phase-split axis: this file already has phase-shaped seams

Separating name/type resolution from expression type-checking is not a rustc-specific idea; it is
the ordinary shape of a front end (parse, then resolve, then check), and rustc itself keeps name
resolution and type-checking as genuinely separate passes over the same source, for the same
reason toylang's own `resolve` layer is already self-contained: resolving what a type name means
does not need to know anything about how expressions are checked, and nothing about expression
checking needs to reach back into how `Str` versus an enum name were told apart. The stream-
linearity and dead-code passes are the same story one layer later: both are separate, single-
purpose walks over an already-built `Tir`, which is exactly the "one pass, one well-defined
transformation" shape nanopass-style compiler architectures name explicitly
([Keep & Dybvig, *A Nanopass Framework for Compiler Education*](https://dl.acm.org/doi/10.1145/1016848.1016878)).
Toylang did not derive this from that literature; it arrived at the same shape by writing four
things that turned out not to need each other.

### The bidirectional-checker axis: `synth`/`expect` is already the standard shape

Bidirectional type-checking splits judgments into two directions — synthesis, "what type is
this?", and checking, "does this match the type I already expect?" — precisely so that forms
whose type cannot be read off their own shape (an empty list, a value read from an unknown
source) can still be typed once *something* supplies the expectation, without falling back to
guessing or full unification
([Dunfield & Krishnaswami, *Bidirectional Typing*, ACM Computing Surveys 2021](https://arxiv.org/abs/1908.05839)).
Toylang's `synth`/`expect` split is exactly this, including the standard fallback rule
(subsumption: if a form can synthesize a type and it equals what was wanted, checking succeeds
for free) at `src/check.rs:2023-2034`. The
[checked-only-forms note](../research-log/checked-only-forms-are-a-class-not-a-lambda-rule.md)
independently rediscovered the same principle the literature gives a name to: `input`, `[]`, and
(after the rework) function bodies, record fields, call arguments, and mapper/conditional bodies
are all forms that denote a *shape* rather than a self-evident type, so they live on the `expect`
side. What [type-flow.md](type-flow.md) does is grow `expect` from three special-cased forms into
a real per-construct match — and every one of those constructs already has a `synth`-side arm in
the same file (`RecordLit` at `src/check.rs:1097`, `Cond` at `src/check.rs:1686`, `Match` at
`src/check.rs:1419`, the call-argument path inside `Call` at `src/check.rs:1225`). Bidirectional
checkers are conventionally organized with a form's two directions living next to each other, not
sorted into a `synth.rs` and an `expect.rs` that both switch on the same AST node — toylang
already does this for `construct` and `collect`, which take a `want: Option<&Type>`-shaped
parameter and branch internally rather than existing as two functions. This matters for the split
decision because it means "one file for `synth`, another for `expect`" is not a phase split at all
in the useful sense — it would separate the two halves of every single construct.

## Pitfalls

### The god-file problem here is structural, not accidental

`src/`'s other five-figure files (`emit_go.rs`, `emit_js.rs`, `emit_rs.rs`, ...) are one file per
backend by construction — the split axis is handed to them for free by there being seven targets
(`src/lib.rs:3-9`). The checker has no analogous multiplicity: every language construct needs
exactly one typing rule, not N per-something rules, so nothing inside `synth`/`expect` repeats the
way emission does per backend. This is the general shape of the "god-file checker" pitfall: a
type checker is the one place in a compiler where the language's entire grammar has to be
enumerated once, so it grows linearly with language surface with no natural seam to split along —
unlike a backend, which grows linearly with target count and gets one file per target for free.
It is also why "by language area" does not have an obvious foothold inside this file the way it
does for the backends: there is no repeated per-area structure to hang files on, only one flat
match over `Expr` that has to stay coherent as a whole.

### Pass ordering and the fused check/lower pass

The one hard sequencing rule already baked into `check` is that every alias and enum must be
resolved, and every function's signature collected, before any function body is checked
(`src/check.rs:71-112`, with the comment at `src/check.rs:108-109` stating why: a definition may
call one that appears later in the file). Nanopass literature calls this out as the class of bug
that reordering or re-scoping passes silently introduces
([Keep & Dybvig](https://dl.acm.org/doi/10.1145/1016848.1016878)): whatever module boundary a
split draws has to keep "resolve everything, then check bodies" as a whole-program step, not
something that can run per-item or get reordered relative to body-checking.

The more specific ordering lesson here is not from the literature; it is
[recorded from this codebase's own history](../research-log/merging-passes-turns-redundant-traversals-into-bugs.md).
Checking and lowering (`Expr` to `Tir`) used to be separate passes and were deliberately merged,
because keeping them apart forced two bad choices: a side table of type information threaded from
check into lower (recorded as "a patch, not a design" in
[the-lowering-needs-types-the-checker-already-computed](../research-log/the-lowering-needs-types-the-checker-already-computed.md)),
and a correctness bug once the merged pass started allocating local IDs (`Ctx::fresh`,
`src/check.rs:63-67`) — a pass that only answers questions can be walked twice for free, but one
that allocates cannot, and nothing in the type system flags a second walk as wrong. This is a real
constraint on any "by phase" split: it rules out separating "checking" from "IR construction"
inside `synth`/`expect`, but it says nothing about the boundary between that fused pass and the
resolve layer before it or the linearity/pruning passes after it, which never allocate and never
race with `synth`/`expect` over the same nodes.

### Error-recovery entanglement: avoided so far, not solved

`Error` (`src/error.rs:6-9`) holds exactly one span and one message, and every checking function
returns `Result<_, Error>` with `?` propagating immediately — there is no accumulation, no
poisoned/error type standing in for "something already went wrong here," and no continuing past a
failure. This sidesteps the specific pitfall rustc hit with `TyKind::Error`: once an error type
is threaded through the checker to represent "already reported, don't cascade," any code that
constructs one without actually reporting an error silently poisons everything downstream of it
([rust-lang/rust#70866](https://github.com/rust-lang/rust/issues/70866)). Toylang has no such
representation to misuse, because it does not attempt multi-error recovery at all: the user sees
one error per run. That is a real, already-made trade-off (simplicity and no poisoning risk,
against never seeing more than one type error at a time), not a gap to fix as part of this survey
— but it is worth naming as the reason recovery-entanglement is not a pitfall toylang has hit yet,
and exactly where it would first appear if multi-error reporting were ever added: whichever module
ends up owning `synth`/`expect` would own that risk too.

## Crates

Both `salsa` and `ena` are compiler-side by construction — neither one's output could reach an
emitted program — so both trivially clear [ADR 0004](../docs/adr/0004-backends-emit-source-run-by-real-toolchains.md)'s
bound. The question is whether either is *well-chosen* against
[the crate-simplification survey](crate-simplification.md)'s standard: a crate earns a place by
closing a real gap, not by being the standard tool for a problem shape this project happens to
share the name of.

**salsa** is an on-demand, incremental computation framework built for exactly the problem the
query-system section above described: recomputing only what changed, across many invocations of a
long-lived process, as in rust-analyzer and chalk
([salsa-rs/salsa](https://github.com/salsa-rs/salsa)). Toylang has no long-lived process and no
incremental-rebuild use case anywhere in the repo. Adopting it would mean restructuring every
checking function into a query (an interned database, `#[salsa::tracked]` boundaries, explicit
inputs) to buy back a capability nothing here asks for. Not recommended — this is the "framework
lock-in" the issue is asking to be judged against, in its clearest form.

**ena** is union-find/congruence-closure, extracted from rustc for unifying type (and region)
metavariables during inference
([rust-lang/ena](https://github.com/rust-lang/ena)). Toylang's type system has no metavariables to
unify: [type-flow.md](type-flow.md) explicitly rules out "bidirectional inference beyond declared
annotations (no guessing, no unification variables leaking into errors)" as out of scope
(`plans/type-flow.md:52-54`), and today's `expect` fallback is a plain equality check
(`&found.ty != want`, `src/check.rs:2024`), not a unification step. Unlike the diagnostics-crate
finding in [crate-simplification.md](crate-simplification.md#not-yet-flagged-for-later-span-rendering-for-diagnostics),
which is genuinely "not yet" pending a feature, `ena` is ruled out by a design decision already on
record — it would need that decision reversed, not just a feature landing, to become relevant.

No other crate — compiler-specific or otherwise — surfaced as closing a real gap in this layer.

## Recommendation

Not a single axis: the file's own seams and the fused-pass lesson point at different answers for
different parts of it.

**Split by phase, first, for the two passes that already don't touch the fused engine.** Type
resolution (`src/check.rs:548-784`) and the post-`Tir` passes, stream linearity plus dead-code
pruning (`src/check.rs:254-488`), are already self-contained: neither shares `Ctx`, neither
allocates alongside `synth`, and neither is where [type-flow.md](type-flow.md) is about to add
code. Extracting both is close to a pure move — together they are roughly 470 of the file's 2049
lines — and it is the lowest-risk piece of this decision because it does not touch the part the
merging-passes lesson says not to split.

**Do not split "by language area" inside what remains.** The remaining core — `Ctx`, `synth`,
`expect`, `access`, `binary`, `construct`, `collect`, and the mapper helpers, roughly 1550-1600
lines even after the phase split above — has no per-area structure to split along the way the
backends do; it is one flat match kept coherent by design, and type-flow's rework specifically
wants each construct's synth-side and expect-side to sit next to each other, which a topic-based
split would pull apart.

**Treat that remainder as a structured exemption, not a file to keep shrinking.** This repo's
sinkhole mechanism ([plans/quality-practices.md, piece 7](quality-practices.md#7-the-sinkhole-rule-adopt-in-principle-last-in-order))
is the vocabulary for exactly this: an exemption that costs a deliberate move and carries a
written argument, rather than a budget nobody expects the file to meet. It has not been built yet
(`sinkhole-machinery`, `plans/board.yaml:305-309`, `status: todo`, blocked only on the already-
`done` `quality-hooks-introduction`). Recommending exemption for the fused core makes this
checker the mechanism's first real case, which the quality-practices survey said explicitly it
was waiting for (`plans/quality-practices.md:217-220`: "a sinkhole rule with no lints that tempt
anyone is machinery without a purpose"). The phase split above is still worth doing regardless of
when or whether sinkhole machinery lands — it shrinks the file and removes two passes that have no
argument for staying, independent of what happens to the rest.
