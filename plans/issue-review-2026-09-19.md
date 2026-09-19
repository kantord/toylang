# Open-issue review, 2026-09-19

Every open issue verified against main at ea5c22a and after, by reading the code, the board and its archive, the docs, and by running the compiler where a behaviour was at stake. The 2026-09-18 status audit had counted 40 issues with only done rows behind them; this review reads each one and says what is true.

Verdicts: 43 close, 9 keep open. The closing comments below are what gets posted when the close sweep runs; none has been posted yet.

## Corrections made with this review

- A duplicate live board row for the stdin-splitting design (already ruled 2026-09-07 and archived) is dropped, and Q35 no longer calls that half open.
- ADR 0011 no longer says the non-UTF-8 line edge is unpinned (#102 ruled and pinned it).
- Euler pages 16, 20 and 25 link the open bigint tracking issue #112 instead of the closed #38.
- plans/string-type-spike.md linked a deleted reference page; it points at the stdin page now.
- The JS web-target refusal named the three retired stdin spellings; it names `stdin` and `dsv`.
- The dev app's run page linked the retired Mail tab; it links the Grill tab.
- A redundant `.into_iter()` in src/check/mod.rs, the code-style bundle index, and an orphan research-log note are fixed.
- Rows filed: declare-terminator-build-2 (#153 was archived done on a lexer-only landing), pipe-through-remaining-backends, docs-gaps-let-tailpipe-destructure-tailcall, benchmark-thesis-publication (#147), matcher-totality-alt-composition-decide (#156), q6-reconciler-decide, opencode-worker-scripts-retire-decide. The transpose row records the compile bug found under #181.

## Verdicts

| # | Verdict | Why |
|---|---|---|
| 93 | KEEP | Euler 23/27 pages are still prose stubs; parked by the 2026-09-01 ruling on a privileged session (row euler-slow-fragments-2) |
| 102 | CLOSE | Ruled 2026-09-03 (refuse), pinned on all seven backends in tests/streaming.rs; ADR 0011's stale sentence fixed today |
| 112 | KEEP | Deliberately unscheduled tracking issue; the Euler skip pages now link it instead of the closed #38 |
| 116 | CLOSE | Fixed in ddbbe5c; shared topsort; snapshot test passes |
| 118 | CLOSE | Superseded by #160, whose four slices all landed |
| 119 | CLOSE | Colon operator ruled and built (3666d38); partial application spun off and ruled |
| 122 | CLOSE | Grill ran, ruled experiment executed, findings in plans/match-type-wrapper-experiment-findings.md |
| 133 | CLOSE | All four Euler pages restored with synthetic inputs; real-data check opt-in |
| 134 | CLOSE | All five sections live at docs/overview/vision.md (411e55f) |
| 135 | CLOSE | toylang slow fence built in tests/docs.rs; Euler 10 and 14 use it |
| 137 | CLOSE | ADR 0001 amended; range streams on all seven backends |
| 139 | CLOSE | Corpus split into eight shards (66ff754); measured 13.6s today |
| 140 | CLOSE | sum/max built and documented; the Float-as-enum side note has no row |
| 141 | CLOSE | Built in ef15c7f; corpus case tail_recursion_deep; contract docs gap filed as a row |
| 142 | CLOSE | first/any/all built in 7024fea across all backends with pages and corpus |
| 143 | CLOSE | Slices clamp; documented at docs/reference/operators/specs.md |
| 144 | CLOSE | Parameter destructuring built (070de8e) on all seven; docs gap filed as a row |
| 145 | CLOSE | Q37 SETTLED, ADR 0007 amended, verified on all seven backends |
| 146 | CLOSE | Second wave landed in 1310dfc: n-body, spectral-norm, mandelbrot on all seven |
| 147 | KEEP | Ruling recorded nowhere; benchmark-plan.md still states the overridden framing; row benchmark-thesis-publication filed |
| 149 | KEEP | Everything built and verified; one acceptance bullet (corpus cases with NaN/Infinity) is the live row float-corpus-cases |
| 150 | CLOSE | let built on all seven (the issue's example returns 8 everywhere); docs gap filed as a row |
| 151 | CLOSE | Sink is a real kind with general containment; jsonlines hand-check retired; sink.md exists |
| 152 | CLOSE | Hoisted match-call form built (call-form-hoist-* rows); documented in functions.md |
| 153 | KEEP | Archived row was a false done: only `;` lexing landed; row declare-terminator-build-2 filed |
| 154 | CLOSE | `|>` built and checker-restricted; zero docs and zero tests filed as a row |
| 155 | CLOSE | Ternary grammar gone; retirement documented in conditional.md |
| 156 | KEEP | Naming ruled and built; the title question (totality, alt-composition, parser combinators) never re-asked; row matcher-totality-alt-composition-decide filed |
| 157 | CLOSE | Fixed in 2d18e3b exactly as diagnosed; verified today |
| 158 | KEEP | Designed and built on Rust; issue text stale; pipe-through-remaining-backends filed; duplicate split row dropped |
| 159 | CLOSE | Superseded by #172, which landed |
| 160 | CLOSE | All four parts built (95bbfbe, ea3f90d, da63f53) and documented (6ad9cb6) |
| 161 | CLOSE | Ruled 'defer until shell-out needs it'; shell-out shipped without it |
| 162 | CLOSE | Built as @(path) routing; the exact shape is a test named for this issue |
| 164 | CLOSE | Fixed (545dba4) and then the Mail tab was retired |
| 165 | CLOSE | Migration done; prelude Opt/Result capitalized 2026-09-19 (df32f9e); verified |
| 166 | CLOSE | visibility map and per-call-site refusal in src/check/mod.rs; tests pinned |
| 167 | CLOSE | @(path) routing built (5766a1e), 29 tests, docs page, Q36 updated |
| 168 | CLOSE | --explain-offload built and documented (docs/reference/cli/explain-offload.md) |
| 169 | CLOSE | Research landed; Q2 settled cartesian and built 2026-09-18 |
| 170 | CLOSE | Addendum written; parent #163 closed |
| 171 | CLOSE | Design ruled 2026-09-08; build parked on the board by ruling |
| 172 | CLOSE | Landed; verified by running; prose tail swept 2026-09-18 |
| 173 | CLOSE | flowStyle() fallback in site/dev/src/lib/flow.ts; MessageCard still live in the Grill UI |
| 174 | CLOSE | Traits, generic impls and colon-only dispatch built and verified; operator-as-prelude-impl left as a revisit |
| 175 | CLOSE | Research landed; Q17 settled 'no separate type'; build is separate and stuck |
| 176 | CLOSE | Research landed; Q22/Q14 ruled; build rows live |
| 177 | KEEP | Go and Rust only; nine live rows; chain stalled on sort-by-max-by-checkpoint-rust |
| 178 | CLOSE | drive-tick.sh deleted 2026-09-11; drive_tick.py resolves by row id |
| 179 | CLOSE | Diagnosis recorded on the archived row; lane mechanism retired |
| 180 | CLOSE | Superseded by plans/status-audit-2026-09-18.md and plans/issue-review-2026-09-19.md |
| 181 | KEEP | All four criteria unmet; plus transpose alone fails to compile on Go and Rust (missing fail helper), added to the row |

## Comments to post

### #93 (keep)

Still accurate as a task, stale in its framing. Option 3 landed: the slow-fragment tier exists (tests/docs.rs, `just slow-test`) and Euler 10 and 14 ship under it. What has not happened is this issue's own work: docs/examples/euler/23 and 27 are still prose-only stubs. The board row euler-slow-fragments-2 is parked at `proposed` because a 2026-09-01 ruling handed it to a privileged manual session (prompt at plans/issue-93-privileged-agent-prompt.md); that prompt found no prior working solution in the repo's history, so both programs are authored from scratch. Page 23 still cites #86 (no Vec sort) as a blocker; sort landed and #86 is closed, and corpus-euler-stale-sort-citations-decide owns that rewording.

### #102 (close)

Decided and built. Round 3 (2026-09-03) ratified option A: a non-UTF-8 byte arriving on the line source is refused, not carried and not replaced. Pinned on all seven backends by assert_refuses_non_utf8 in tests/streaming.rs (landed 6920c86); re-run today, all pass. The Rust tl_fail omission found while verifying this was #157, also landed. ADR 0011's paragraph that still called this edge unpinned was rewritten today. The construct itself has since been renamed: the three stdin keywords collapsed into one `stdin` source (#172). Closing.

### #112 (keep)

Still accurate and still deliberately unscheduled (row bigint-tracking, prio 5, archived as tracked-not-scheduled). Two updates: the survey plans/bigint-tracking-research.md now exists, putting the Python, Java, Go and JS models side by side on the Int64 precedent; and the three blocked Euler pages (16, 20, 25) linked the closed #38 rather than this issue, which was fixed today so the skip links land on something open.

### #116 (close)

Fixed and pinned. ddbbe5c lifted the ordering into a shared topsort and ran printers() through it, so two enums that reach each other through Vec get toylang's own refusal naming both printers instead of jq's raw error (src/emit_jq.rs printers and cycle_message). tests/backend_jq.rs a_genuine_cycle_between_printers_is_refused_cleanly pins it; re-run today, passes. Row jq-printer-cycle-refusal is archived done. Closing.

### #118 (close)

Superseded by #160. The decide session this asked for ran (row js-node-web-split-design) and ruled one system, all four together, filed as #160; all four slices have landed: Node/Web is one emitter with a target flag (src/emit_js.rs), the .d.ts ships beside the .js with a tsc gate (docs/reference/cli/build.md), the web escape hatch is declared in config with a compile-time refusal when missing (docs/reference/cli/config.md), and toylang.conf.yaml autoloads by walking upward. The one sub-ask not built is a docs page showing all four output combinations; it belongs on #160 if still wanted. Closing.

### #119 (close)

Answered and built. Round 2 picked the colon operator: `x:foo(y)` is the only spelling and `.` stays projection. The curry thread was reframed and spun off as partial-application-system-design, since ruled (option C: both shapes are one mechanism once functions are first-class, always syntactically explicit). Colon calls, receiver-typed multi-impl dispatch and backend name mangling landed in 3666d38; tests/corpus/colon_call_ufcs_sugar.yaml pins `3:double()` -> 6 on every backend. The remaining gap, first-class functions, is tracked on the board (closures-first-class-functions-design). Closing.

### #122 (close)

The grill round this asked for ran and ruled: build the full Match<T> wrapper as an experiment first. The spike ran and wrote plans/match-type-wrapper-experiment-findings.md, which answers the three open threads: the wrap point is one site in pipe() but a guard arm's body sees `.` as the enum subject, so the two wrap decisions differ; the generic-impls pillar had no trait table then (multi-impl dispatch has since landed separately); 'applies only past a passing guard' is not decidable at check time; and the alias-result-struct is new TIR plus a change in every emitter. Nothing is on the board for it. Closing as the record; if the wrapper is still wanted it needs its own build row sized as that multi-backend change.

### #133 (close)

Done. docs/examples/euler/08, 11, 13 and 18 each carry a real toylang fragment checked against a small synthetic input, with prose pointing at tests/euler_real_data.rs and `just euler-data DIR` for the real-size verification, exactly the shape proposed here and within #39's no-real-data rule. Problem 11's blocker cleared (#132 closed; the Python emitter raises its recursion limit). Row euler-pages-restore is archived done. Closing.

### #134 (close)

Done in 411e55f. What this is, Two guiding principles, Values, Two worked programs and Non-goals live at docs/overview/vision.md and are gone from draft.md; the worked programs are labelled vision rather than documentation and name the unbuilt features they use; the jq-fork history is one sentence deferring to ADR 0002. Closing.

### #135 (close)

Built. tests/docs.rs implements the `toylang slow` fence: the default run type-checks and emits a marked fragment on every backend and skips only its execution, and `just slow-test` runs them. Both constraints are enforced: a slow fragment must carry an output fence, and the harness fails if no fragment is marked slow. Euler 10 and 14 landed under it with measured timings. Row docs-slow-fragment-tier archived done. #93 is held by its own ruling, not by this mechanism. Closing.

### #137 (close)

Both halves done. docs/adr/0001 carries the 'Amendment: range joins the sources (#137)' section answering the Q13 reopening; tests/streaming.rs pins a per-backend *_streams_range test on all seven; range is documented as `Int -> Stream<Int>`; the Euler pipelines fuse. Closing.

### #139 (close)

Done and measured. tests/corpus.rs exposes eight shard tests (66ff754); re-run today: slowest shard 13.5s, 13.6s wall, against the 47-67s baseline. No behaviour change, corpus data untouched. Row corpus-test-split archived done. Closing.

### #140 (close)

Ruled, built, documented and pinned: sum is Vec<Int> -> Int / Vec<Int64> -> Int64, max returns Opt on empty, both with fences the docs harness runs on all seven backends, and the no-min/no-product discipline is enforced by refusal tests (tests/reductions.rs). The IEEE/JSON note was settled separately as Q37 (#145, built under #149). The second note, an internal Float-as-enum representation plus a survey, was never picked up and has no row; file fresh if still wanted. max_by (#177) is on Go and Rust only. Closing as the ruling record.

### #141 (close)

Ruled and built. ef15c7f lowers self-tail-calls to loops on js and py, keyed off tir::has_tail_call, with the carve-out that a partial match's arm bodies are not tail positions. tests/corpus/tail_recursion_deep.yaml requires all seven backends to agree on a 100,000-deep countdown; it passes. The contract is written nowhere a user can read it, so that is on the board as docs-gaps-let-tailpipe-destructure-tailcall. The wider recursion-to-loops question was never picked up and would need its own issue. Closing.

### #142 (close)

Built and closed. first, any and all landed as the search cuts in 7024fea across every backend, with reference pages that state the cut vocabulary and nine corpus cases. Rows first-search-cut and search-layer-build archived done. Extending first to Stream is owned by the later search-cut-semantics ruling (search-and-fold-design), not this issue.

### #143 (close)

Built and closed. `v[a:b]` clamps out-of-range bounds (`[1,2,3][1:9]` is `[2,3]`) and composes with Vec `+` as required; bounds, negative bounds and the `[:]` refusal are documented at docs/reference/operators/specs.md. Row slices-build archived done. The maintainer's aside about slice syntax on streams never got a row; it is a new design question, so file it if still wanted.

### #144 (close)

Built and closed. Match-arm-style record parameter destructuring landed in 070de8e and runs identically on all seven backends (`fn g({a, b}: {a: Int, b: Int}) -> Int = a + b` returns 5 everywhere); the annotation stays explicit as ruled. Corpus: param_destructure.yaml and param_destructure_rest.yaml. docs/reference/syntax/functions.md never mentions the form; that is on the board as docs-gaps-let-tailpipe-destructure-tailcall.

### #145 (close)

Ruled, recorded and built. Q37 in plans/questions.md reads SETTLED (gh:145, built under gh:149) and docs/adr/0007 carries the amendment. Verified today: `[1.0/0.0, 0.0/0.0, -1.0/0.0, 1.5+2.25]` prints `[Infinity,NaN,-Infinity,3.75]` identically on lua, js, jq, go, py, rust and llvm, while Int `1 / 0` still raises. Printing is ECMA-262 Number::toString everywhere (docs/reference/types/float.md). Closing.

### #146 (close)

The deferral expired and the wave landed. Float exists with settled semantics (#145, #149); n-body, spectral-norm and mandelbrot landed in 1310dfc as benches/programs entries with corpus cases, on all seven backends; n-body reproduces CLBG's published energies. The gaps the wave hit (no sqrt or Int-to-Float bridge; the formatter dropping interior comments) are their own board rows. Closing.

### #147 (keep)

Still open, and the one ruling in this batch never written down or acted on. Row benchmark-goals is archived done, but its title is the only record; plans/benchmark-plan.md still states the timings are 'comparative color across this project's backends ... not a claim about toylang in the abstract', the framing this ruling overrode, and nothing publishes benches/results. Board row benchmark-thesis-publication now carries the three pieces: rewrite the plan's thesis, add a publication surface, carry the CLBG caveat there.

### #149 (keep)

Nearly done. Literals, arithmetic, comparisons and the non-finite values agree byte for byte on all seven backends, and the 'not yet settled' printing question is settled (Q37, ADR 0007 amendment). Fourteen board rows are archived done. What remains is the last acceptance bullet: no corpus case carries NaN or Infinity yet; that is row float-corpus-cases, currently in a repair dispatch. Close when it lands.

### #150 (close)

Built and closed. The two-binding example in this issue returns 8 on all seven backends, with no `in` keyword. Rows let-bindings-build and input-type-annotation-build archived done; corpus let_classify_cond.yaml and let_multi_input_annotation.yaml. `let` has no reference page; that is on the board as docs-gaps-let-tailpipe-destructure-tailcall.

### #151 (close)

Built and closed. `fn write_all(v: Vec<Str>) -> Sink = jsonlines(v)` runs, a sink in value position is refused by the general rule, and the hand-special-cased jsonlines position check is retired (src/check/mod.rs records it as folded into the rule). Reference page docs/reference/types/sink.md runs its fences on all seven backends. Row sink-kind-build archived done.

### #152 (close)

Direction ratified, spelling grilled, built. The follow-up round produced signature-matching-deeper-research and the four-step call-form-hoist build, all archived; `fn render = Msg(Ping -> ... or Text{body} -> body)` compiles and `render(text({body: "hi"}))` prints hi. Documented at docs/reference/syntax/functions.md. Closing.

### #153 (keep)

Still open, and the board was wrong about it. Row declare-terminator-build was archived done on 8c84499, a merge whose only code commit is 04aa00a, 'Lex `;` as a token', which says the parser does not consume it yet. Confirmed today: Tok::Semicolon appears only in the token table, `fn id(s: Str) -> Str = s;` is refused with 'expected an expression, found `;`', and the same-line heuristic this ruling retires is still live and still taught in docs/reference/syntax/functions.md. The ruling is recorded nowhere but here. Board row declare-terminator-build-2 now carries the remainder.

### #154 (close)

Built and closed. `|>` is a real token and production (Tok::PipeGt, Expr::TailPipe), the checker restricts its callee to a sink, and the jsonlines position hand-check is retired. Verified `["a"] |> jsonlines` on all seven backends. A fixed-string grep for `|>` hits only src/: no corpus case, no test, no docs page uses it. That is on the board as docs-gaps-let-tailpipe-destructure-tailcall.

### #155 (close)

Built and closed. The ternary production is gone (`"big" if 10 > 5 else "small"` fails to parse), the retirement and the before/after pair are documented at docs/reference/operators/conditional.md, and no toylang fence in docs/ uses the form. Row retire-ternary-build archived done.

### #156 (keep)

Both threads in the body are resolved: round 2 ruled curly-only patterns and capital-first variants (built; the prelude followed on 2026-09-19 with Some/None/Ok/Err), and the scrutinee-naming thread was answered by the #152 call-form hoist. The question in the title was never re-asked: the row closed on the naming ruling alone, Q30 still reads LEANING, and the compiler still refuses `o | Some{v} -> v or None -> 0` over Opt with 'how its arms compose is still being decided'. Board row matcher-totality-alt-composition-decide now carries that remainder; this issue should be read as that question only.

### #157 (close)

Fixed and closed. `uses("tl_read_lines(")` was added to the fail computation in src/emit_rs.rs with a comment recording why the scan could not see it (2d18e3b). Verified today: a stdin-only program compiles and runs on the rust backend. The reproducer's `lines` spelling is retired (#172), but bare `stdin` emits the same helper, so the fix is still doing work.

### #158 (keep)

Designed and built on Rust, text stale. The Q35 dependency landed (three rulings, plans/questions.md Q35), pipe_through landed in fcea805 on the Rust backend, and the other six backends refuse cleanly. The ratified shape here, `lines | pipe_through("grep", ["foo"]) | collect`, is not what was built: the landed signature is `pipe_through({cmd: Str, args: Vec<Str>, lines: Stream<Str>}) -> Stream<PipeLine>` (docs/reference/builtins/pipe_through.md). Two records fixed today: a duplicate live board row for the already-ruled stdin-splitting question was dropped, and pipe-through-remaining-backends now tracks the six missing arms the way transpose and sort_by already had rows.

### #159 (close)

Superseded by #172, the re-file created 2026-09-02 after the ruling to drop the poisoned issue-159 lane. The board records the split (stdin-redesign-build abandoned with its escalation note; stdin-redesign-build-2 archived as landed). The ruling is built: `input`, `inputs` and `lines` are undefined and `t(parse(stdin))` works. Closing.

### #160 (close)

All four parts built and documented. js-node-web-split-build, ts-type-generation-build, js-web-escape-hatch-build and toylang-conf-yaml-build are archived done; the unconditional fs read is now conditional on target and substitute, and a web-target program reading stdin with no substitute is refused at compile time. docs/reference/cli/config.md and build.md cover the config file and the .d.ts. The process note about self-decomposing design tasks has no row; file it against the board schema if wanted. Closing.

### #161 (close)

Ruled and deferred on 2026-08-31 (65c126f): defer template strings until shell-out needs a capture-literal spelling. Shell-out has since shipped as the ordinary call pipe_through, so the trigger is gone; no design, question number or row exists. Closing as deferred with no trigger; reopen if a feature needs interpolation.

### #162 (close)

Built. The matcher-arm-to-submodule idea is the ruled `@(path)` routing-arm form, implemented in 5766a1e; tests/module_routing.rs matcher_arms_route_to_different_modules is the exact shape sketched here and agrees on all seven backends. Documented at docs/reference/syntax/modules.md. Closing; named imports/exports stay on Q36.

### #164 (close)

Fixed and then obsoleted. Per-round fetch isolation landed in 545dba4/8378549 (row mail-rounds-isolation); the Mail tab was then retired for forest rounds (6a4b84b), and the Grill UI fetches each topic's round as its own query, so one bad file cannot blank the list. Closing.

### #165 (close)

Done. The corpus/docs migration landed under variant-types-flip, and the last exemption, the prelude's Opt and Result, closed on 2026-09-19 in df32f9e: `pub enum Opt<T> { Some(T), None }` and `pub enum Result<T, E> { Ok(T), Err(E) }`, constructors still lowercase, exemption gone; 61a1803 makes a lowercase arm head read as a guard. No lowercase matcher spelling survives in docs/, README or the corpus. Closing.

### #166 (close)

Done. Every definition carries its file (Origin, widened to Origin::Module for routed files) and visibility is enforced per call site: a non-pub call from a foreign file is refused with '`X` is not `pub`, so it can only be called from its own file' (src/check/mod.rs, comment cites gh:166). Pinned by tests/module_routing.rs; Q36 records it. Closing.

### #167 (close)

Built. `@(path)` landed across four steps with the semantics in 5766a1e: a routed arm loads the file, merges its definitions under Origin::Module, and applies its `handle`; 29 tests in tests/module_routing.rs run on every backend; documented at docs/reference/syntax/modules.md; Q36 updated. Note the landed form quotes the path, `@("route-groups/baz.toy")`. Closing.

### #168 (close)

Built and documented. The flag is a leading flag on run/emit/build (src/main.rs), the diagnostic lives in src/offload.rs and reports to stderr one line per decision; documented at docs/reference/cli/explain-offload.md with fragments the harness runs; Q8 records it. Editor hover (option C) was left unscheduled by the ruling itself. Closing.

### #169 (close)

Research landed as plans/jq-expressiveness-under-zip-or-explicit-research.md and fed the re-ask; Q2 settled on the cartesian default and binary-op-cartesian-build landed 2026-09-18 on all seven backends. Closing.

### #170 (close)

Done. plans/erlang-target-research.md carries 'Addendum: empirical verification is not possible on this host (gh:170)': erl/erlc/escript are absent, a missing-dependency gap rather than the sandbox wall this issue suggested, and the recommendation never depended on it. #163 was closed on that basis. The concurrency flag is board row concurrency-open-item-decide. Closing.

### #171 (close)

The design is done: plans/http-query-sugar-research.md plus the 2026-09-08 ruling (three TLS-capable backends only, `fetch(url)` returning a full response record). The build is board row http-query-sugar-build, parked by a 2026-09-09 ruling for a privileged manual session with the prompt at plans/issue-171-privileged-agent-prompt.md. Closing the design issue.

### #172 (close)

Landed. `input`, `inputs` and `lines` are retired into `parse(stdin)`, `stdin | map(parse(.))` and bare `stdin`; verified on main. Row stdin-redesign-build-2 archived done; the 2026-09-18 audit swept the fourteen prose sites and the README example that still taught the old names. ADRs keep the old spellings as records. #159 closes as superseded by this. Closing.

### #173 (close)

Fixed. The wash-table lookup moved behind flowStyle(), which falls back to a neutral style for an unknown flow value (comment names this incident); MessageCard no longer indexes FLOW[flow] directly, and an error boundary landed separately. The fix is live in the Grill forest UI even though the Mail tab was retired. Closing.

### #174 (close)

Built. Rust-shaped trait and impl declarations parse, check, dispatch and emit on every backend; generic impls over type constructors work; colon-only is enforced by a refusal. Verified `{r: 3}:area()` -> 9. Two rows delivered it (trait-interface-build, then trait-interface-dispatch-build after the audit found the first was parse-only). The prelude now uses the mechanism (trait Fold). Ruling item 2, builtin operators as prelude impls, was tentative and is unbuilt with no row; file fresh if worth re-asking. Closing.

### #175 (close)

The research landed (plans/dense-tensor-design-research.md) and the re-ask ruled on 2026-09-08: no separate tensor kind, Vec is the tensor-capable type, `tensor(n; m)` constructs, rows on `.[]`, hard-fail nulls. Q17 records it. What remains is build: transpose on five backends is #181, and tensor-constructor-build is stuck behind tensor-constructor-build-convergence-ruling. Closing the research issue.

### #176 (close)

Research landed (plans/select-materialization-research.md) and the re-ask ruled 2026-09-07: a lazily built selection vector or mask makes the result indexable before materialization, it is not a distinct type (Q22: the mask lives behind Vec), and compaction happens on the first strong reference. The build is boarded: Python landed, six backends plus the umbrella wait on select-lazy-materialization-convergence-ruling. Closing the research issue.

### #177 (keep)

Still open and accurate, partially built. sort_by and max_by run on Go and Rust (src/backend_support.rs is the source of truth) and are refused cleanly elsewhere instead of panicking. Lua, JS, Python, jq and native remain, boarded one backend per commit with a checkpoint between; the chain is stalled at sort-by-max-by-checkpoint-rust, a ready decide row with no round composed since 2026-09-15. One correction: the Problem 22 unblock claimed here does not hold; its real blocker is missing name data.

### #178 (close)

Obsolete: the tool no longer exists. drive-tick.sh and dispatch-worker.sh were deleted in a504736 under the ruling that simple_dispatch.py is the only dispatch mechanism; drive_tick.py resolves a delegated row by row id against dispatch-log.csv and never derives an issue-numbered worktree path. Row drive-tick-lane-resolution-bug archived. Leftover lane worktrees are surfaced under stale-worktree-cleanup-decide. Closing as obsolete.

### #179 (close)

Answered and obsolete. The diagnosis is recorded on row stuck-lane-investigation-chain-diagnosis: workers did start on all three; two died to machine interruption and were redispatched, one traced to webfetch-blocked runs and was landed directly; no systemic bug. Both research files exist. The lane mechanism was retired on 2026-09-11. Closing.

### #180 (close)

Superseded. Every count here is stale (52 open issues; board.yaml references 9 issue numbers). The 2026-09-18 status audit redid the sweep, and plans/issue-review-2026-09-19.md verifies every open issue against main with a verdict each. The per-issue close decisions are that review plus board row issue-hygiene-close-sweep-decide. Closing in favour of those.

### #181 (keep)

Still accurate; all four acceptance items are unmet. One detail is out of date: the other five backends no longer hit unreachable!, they are refused cleanly by src/backend_support.rs. Bug found while verifying: `transpose([[1, 2], [3, 4]])` alone does not compile on Go (`undefined: tlFail`) or Rust (`cannot find function tl_fail`), because the transpose helper calls the fail helper without pulling it in; it is masked whenever anything else uses it. The corpus case would have caught it, so it goes first; the row transpose-remaining-backends now records this.
