# Status audit, 2026-09-18

Prompted by a wrong answer. Asked whether the language was usable, a session checked the
corpus filenames and `docs/reference/types/` and concluded that `Float` did not exist. It has
existed on all seven backends since 2026-09-06, under fourteen board rows and about fifty
per-backend tests; what did not exist was a corpus case, a reference page, or a `questions.md`
entry that said so. This audit went looking for every other place the records had fallen
behind the code, and the reverse. Four sweeps, each verified by reading or running rather than
by trusting a row's title: implementation against docs, board against reality, the design
record against the code, and the compiler's own warnings.

## What is built

A compiled, typed jq dialect with records, enums with exhaustive match, generic enums, `type`
aliases, `Opt` and `Result`, `Int`, `Int64`, `Float`, `Str` as Unicode scalar values with
`Char`, streams born at `stdin` or `range` with fused read-transform-write loops, `let`,
traits with generic impls and colon-call dispatch, `|>` sink application, `Seq<H, R>` as a
checker type, 23 reserved builtins plus `transpose`, a formatter, a TextMate grammar, and
seven backends that must print identical bytes on every corpus case (242 cases) and every
docs fragment. The suite is 444 tests and it is green on `b35729b`.

It is not usable as a jq replacement. The builtin set has no `group_by`, `unique`, `keys`,
`has`, `to_entries`, `add`, `reduce`, no string splitting or search, and no regex; the string
representation decision (ADR 0011) rules out length and indexing on purpose, so those come
back as builtins or not at all. Several ruled designs have no build row: cartesian `Vec op
Vec` (Q2), the `=` update with `One` (Q3), first-class functions and partial application (Q33
and the `.`-rebinding ruling), `Batch<T>` (Q21), the lazy `select` representation on six
backends (Q14), and `tensor(n; m)` (Q17). `sort_by`, `max_by`, `transpose` and `pipe_through`
exist on one or two backends and panic inside the emitter on the rest.

## What was wrong in the records, and what this commit did about it

**Retired source spellings.** `input`, `inputs` and `lines` were retired into `parse(stdin)`,
`stdin | map(parse(.))` and bare `stdin` by the stdin redesign (gh:172). Every code fence had
been migrated, because the docs harness runs fences; the prose around them had not, because
nothing runs prose. Fourteen sites across the tutorial, three guides, four reference pages,
`CONTEXT.md` and `plans/questions.md` taught the dead names, and the README's first example
did not compile. All rewritten here. Three ADRs (0001, 0010, 0011) still use the old
spellings as live ones; they are decision records and were left as written.

**Float.** No reference page, no corpus case, and `questions.md` said printing was "still
open" and waiting for a real program. Added `docs/reference/types/float.md` with fragments
the harness runs on all seven backends, amended ADR 0007 to record that printing and the
non-finite values are decided and built, and rewrote Q37. Writing the page found a wider gap
than the one recorded: on jq, any `Float` inside a `Vec` or record skips the relayout, so
`[1.0e-7, 1.0e21]` prints `[1E-7,1000000000000000000000]` there, not only the non-finite
values the jq row had pinned. Closed the same evening: emit_jq.rs renders any Float-bearing
structure as JSON text around its own float formatter instead of handing it to jq's encoder.
Row: `float-corpus-cases` (still open, dispatched). The formatting sweep then found a
second one: `toylang fmt` spelled Float literals through the backends' shortest-digits
helper, so `2.0` came back as `2` and `1.0e21` as a 22-digit run, both of which lex as
`Int`; a formatted program changed type or stopped compiling. Fixed in `emit_toylang` with a
test in `tests/fmt.rs`. Neither bug could surface before, because no docs page and no corpus
case carried a Float.

**Rulings never written back.** Twenty-three of the forty-two questions carried a status
that the board had already overtaken: Q3, Q8, Q14, Q16, Q21, Q22, Q23, Q33 were ruled in
grilling rounds during the first week of September and still read LEANING or OPEN; Q34, Q36
and Q42 said things were unbuilt that had shipped (`type` aliases, file-scoped privacy,
`fields`); Q2, Q17 and Q18 described a ruling without saying it was unbuilt or only partly
built. Every table row and detail section is rewritten from the board rows that carry the
ruling. Q3 and Q6 had a heading and no body; Q3 now has one, Q6 is still empty because there
is nothing recorded anywhere to put in it.

**Glued commas.** Six pages (`max_by`, `pipe_through`, `functions`, `join`, `join_lines`,
`specs`, and the prelude index) had lost the space after commas and colons in prose, and the
prelude index had "anda" and "andthe". A worker's output, not a style; repaired.

**The prelude page** listed two functions and two enums. The file holds three enums, a
trait and an impl as well, and the page's grammar claim ("fn or enum and nothing else")
was false. Rewritten.

**`pipe_through`, `sort_by`, `max_by` pages.** `pipe_through.md` said the other backends
"refuse to compile" a program using it; they panic (`unreachable!` in six emitters). The
`sort_by` and `max_by` pages said nothing about running on Go and Rust only. All three now
say what happens. The clean-refusal fix is a row, `unbuilt-builtin-arms-refuse-not-panic`.

**README.** `cargo run --quiet -- run ...` failed on every example because the crate has
three binaries and no `default-run`; added `default-run = "toylang"` to `Cargo.toml`. The
`adults` listing used `input`; the `shapes` listing drifted from the file. The CLI's usage
string omitted `rust`, a backend the README itself loops over.

**Code.** `resolve_defs` in `src/check/mod.rs` took a `file: Origin` parameter that nothing
read, left over from the 2026-09-06 extraction; removed. Two comments cited "Q2 is open" and
"Q20 in draft.md"; corrected. `sort_by_and_max_by_handle_str_and_int64_keys` in the Go and
Rust backend tests ran two programs and asserted nothing (the Rust brief said to port the Go
test as-is, and the Go test had the hole); both now assert.

**Root files.** `ONE_OFF_FIXES.md` (tracked, 2026-09-04) recorded three manual fixes, two of
which landed the same week and the third of which already lives on the
`euler-slow-fragments-2` row; deleted. `LAND-FAILURE.txt` was an untracked, gitignored
landing note for gh:172, which landed; deleted.

**Board.** Twenty-four rows sat at `status: done` in `plans/board.yaml`, the oldest for
fifteen days, when the convention (issue #113) is that a landed row moves to the archive.
Nothing enforces that; `board-lint.py` only checks the archive side, and `dispatch_state.py`
happens to ignore done rows when resolving `needs`, so no dispatch was blocked, but the drive
skill's prose says the opposite. All twenty-four archived with `board-archive.py`; the lint is
a row, `board-lint-done-in-live-board`. Two live rows said `max_by` had landed "only on the
Go backend" (Rust landed 2026-09-15) and one, `select-lazy-materialization-convergence-ruling`,
still asks the maintainer to rule on a stall that its own option (a) partly resolved when
Python landed; both corrected in place, the round file left alone.

## Follow-ups that were implied and never filed

Each of these is a sentence on a done row that named work still to do; none had a row, an
issue, or a mention anywhere else. Filed today, in the order they appear on the board:

- `float-corpus-cases`, `float-jq-nested-in-container` (from the two jq and native float
  rows).
- `transpose-remaining-backends`: `tensor-transpose-build` is archived done while its own
  body lists five backends and the corpus as remaining. `transpose` is also missing from
  `BUILTIN_NAMES`, which is why the docs gate never demanded a page and why a user can define
  a function of that name and have it silently shadowed.
- `module-routing-semantics-build-2`: `module-routing-semantics-build` was landed and
  archived as done after a run that ended on `max_turns` with the mechanical half done (the
  `Origin` widening, 29 lines) and none of the semantics. `Origin::Module` is never
  constructed and `@(path)` is still refused. The suite stayed green because the missing
  half has no test, which is the general lesson: GREEN-on-max_turns is not done.
- `seq-runtime-pair-emission` (from `seq-type-primitive-build`).
- `benchmark-second-wave` (gh:146 deferred three benchmarks "once Float exists").
- `binary-op-cartesian-build` (Q2 ratified, checker unchanged, no row).
- `closures-first-class-functions-design`: three rulings now wait on first-class functions
  and `dsv-partials-migration` is parked on a build row that does not exist.
- `stdin-stdout-splitting-design`: flagged in every Q35 round, never asked.
- `concurrency-open-item-decide` (maintainer note on the Erlang row).
- `docs-completeness-gates`: the one existing gate covers builtins only; types, tags and the
  README each need one, and each would have caught Float on its own.
- `issue-hygiene-close-sweep-decide`, see below.

## Things that look broken and are not

**Thirty rustc warnings on every build.** Twenty-nine come from `build.rs`, which
`#[path]`-includes six source files as its own modules and calls one function, so
everything else in them is dead from the build script's point of view. The Stop hook already
drops `custom-build` messages (gh:81); `cargo build` does not. Nothing in the repo gates on
warnings at all (`just check` runs tests only, `just clippy` has no `-D warnings`), which is
why the one real lib warning survived twelve days. Left as is; a `[lints.rust]` gate would
need a build-script carve-out first.

**A red suite on main.** `just test` failed ten tests at first: eight `aliases` snapshots and
two jq refusals, all reporting their source as a `js-wt` directory under a deleted scratchpad.
A Claude session had made a scratch worktree at `00cec45`, built its test binaries into this
checkout's shared `target/`, and gone away; cargo did not rebuild them for main, and the
stale binaries looked for their snapshots in the dead path. Touching the test sources and
rebuilding gave 444 passed. The worktree was prunable and is pruned. Worth a rule: a scratch
worktree that shares `target/` with main leaves main's next test run reading the wrong
binaries.

## Open GitHub issues with nothing live behind them

Forty open issues are referenced only by done rows: 102, 112, 116, 118, 119, 122, 133, 134,
135, 137, 139, 140, 141, 143, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161,
162, 164, 165, 166, 167, 168, 169, 170, 172, 173, 174, 178, 179. Three of them (149, 158, 167)
gained live rows today. 159 was dropped and refiled as 172, which landed; both are open. 118
is superseded by 160; both are open. 178 and 179 concern tooling retired on 2026-09-11. Six
open issues have no row in either board file: 142, 144, 145, 146, 147 (all "Ruling:" record
issues; 146 gained a row today) and 180, the previous audit's own tracking issue, whose
counts are now stale. `dsv-partials-migration` points at gh:136, which is closed. Which of
these close and which stay open as records is the maintainer's call:
`issue-hygiene-close-sweep-decide`.

## Left alone, on purpose

- `draft.md` carries a dozen superseded sections (the search-operator table, reader
  batching, the `@f32` tensor kind with an Arrow bitmask, `cell` mutation, a "see the end"
  that points at nothing). `draft-md-cleanup-review` already owns deleting it; rewriting it
  first would be wasted.
- ADRs 0001, 0010 and 0011 use the retired source spellings. ADR 0007 got an amendment
  because its "not decided here" list was decided; the others are only using old names for
  the same things, and an amendment per spelling would be furniture.
- `toylang.conf.yaml`, `--explain-offload`, and the `.d.ts` that `build FILE js` emits are
  undocumented outside `src/`. Real gaps; not pursued here because each needs a page
  written from running the feature, not from the audit.
- Two ready decide rows have been gating twenty-one build rows for days:
  `sort-by-max-by-checkpoint-rust` (ready since 2026-09-15, gates ten) and
  `mutation-rule-v1-checkpoint-rust` (ready since 2026-09-17, gates eleven). Neither has a
  forest round composed. `convergence-guard-round-composition` is being built for exactly
  this and was GREEN and unlanded at the time of writing; composing the rounds by hand would
  race it.
