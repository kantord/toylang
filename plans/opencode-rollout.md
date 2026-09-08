# The opencode delegation rollout log

Maintainer ruling, 2026-08-30: claude-code-based delegation is **retired**. All new
delegated build work dispatches through opencode + DeepSeek V4 Flash
(`.claude/scripts/opencode-worker.sh`; the enwiro-delegate skill carries the flow).
No new delegated work happens with claude code until the re-evaluation -- in-flight
claude lanes at ruling time finish and land normally.

This file is the coordinator's OBSERVABILITY OBLIGATION for the rollout: the flip is
provisional, and the evidence for keeping or reverting it accumulates here, not in
anyone's memory.

## Re-evaluation gate

After roughly **30 landed opencode lanes** (count: lanes.csv rows with kind=worker and
a deepseek model, cross-checked against the archive), the coordinator boards a `decide`
row -- `opencode-rollout-review` -- attaching this log and the cost/quality comparison
against the pre-rollout claude baseline in `~/.cache/toylang-drive/lanes.csv`. Until
that ruling, the default stays opencode.

## Incident log (append-only; the coordinator records EVERY issue, small or large)

Record at minimum: date, lane/issue, what went wrong, what it cost (retries, review
findings, coordinator interventions, abandoned work), and whether a claude worker
would plausibly have avoided it. Landings with zero incident need no entry -- absence
of entries over many lanes is itself the finding.

| date | lane | what happened | cost | claude-proof? |
|------|------|---------------|------|---------------|
| 2026-08-30 | issue-88 (csv-inputs-idea, gh:88) | Worker correctly diagnosed the task as a design decision (DSV delimiter vs. nullary sources) rather than a build, but its own bash call to file the follow-up issue got auto-rejected by the permission gate, so it exited after 14 steps with zero commits and no issue filed -- a dead lane with no visible trace besides its event log. Coordinator posted the analysis to gh:88 and reclassified the board row to `decide` by hand. | $0.01, 14 steps, one coordinator intervention (comment + reclassify) | No -- the permission auto-reject would block a claude worker's `gh issue create` too; not opencode-specific. The board row was simply mis-scoped as `build` when it should have started as `decide`. |
| 2026-08-30 | issue-98 (builtin-renames, gh:98) | 80 steps of correct, complete work, but the final `INSTA_UPDATE=always just check` got permission-rejected, leaving two pending `.snap.new` files uncommitted. Coordinator reviewed both diffs by hand (exactly the rename, nothing else), accepted them, re-ran `just check` green, and landed as-is. | $0.12, 80 steps, one coordinator review-and-accept | Partial -- a claude worker under the same auto-permission classifier would hit the same rejection on an env-var-prefixed command; the work itself was opencode/DeepSeek-quality-fine. |
| 2026-08-30 | issue-129 (euler-data-problems-unblock, gh:129) | 23 steps building the right opt-in-check design (matches the issue's own suggested shape), but a live `curl` to projecteuler.net to self-verify its hardcoded expected answers got permission-rejected, and the worker exited with the test file uncommitted, `justfile` recipe uncommitted, no ESCALATION.md. Coordinator fetched the real official Project Euler data directly (network access available outside the worker sandbox), ran the check, and found two real bugs in the worker's file along the way: missing the trailing newline every other `Expect::Output` case in the repo carries, and problem 13's expected value written as the human-published digit string rather than the `Vec<Int>` the program actually (correctly) prints. Fixed both, verified 3/4 problems against real data, filed gh:132 for the 4th (a genuine, previously-unknown Python backend limitation the real data surfaced), landed. | ~$0.03 (worker) + one coordinator data-fetch-and-fix session, 2 new follow-up issues filed | No -- the network-fetch rejection is sandbox policy, not opencode-specific; the two content bugs are ordinary worker mistakes a claude worker could equally have made. |
| 2026-08-30 | issue-108 (benchmark-synthesis, gh:108) | Worker read both benchmark spike docs, then tried `gh issue list --state all --search "benchmark"` for cross-referencing context; permission-rejected, and it gave up entirely rather than proceeding with what it already had -- exited after 6 steps, zero commits, no ESCALATION.md. | $0.003, 6 steps, lane sat idle until the coordinator noticed and re-dispatched | Unclear -- the give-up-on-first-rejection behavior, not the rejection itself, is the finding; whether a claude worker would have persisted through an equivalent auto-reject is untested. |
| 2026-08-30 | issue-125 (benchmark-spike-citations, gh:125) | Worker tried two `webfetch` calls (TechEmpower's repo, SPEC's license page) to source the exact citation text; both permission-rejected, and it gave up -- exited after 4 steps, zero commits, no ESCALATION.md. | $0.002, 4 steps, lane sat idle until the coordinator noticed | Unclear, same shape as issue-108 -- a task needing external-web sourcing hit the same wall a claude worker's WebFetch would likely also hit under an equally strict allow-list. |
| 2026-08-30 | issue-116 (jq recursive-enum printer cycles, gh:116) | 25 steps: implemented the printer-cycle fix in `src/emit_jq.rs`, ran `just check` green on the existing suite, then a scratch `mkdir /tmp/opencode/check && cat > ... <<'EOF'` heredoc it wanted for its own manual verification got permission-rejected. It kept going (wrote the new test and a snapshot in `tests/backend_jq.rs`) but exited before running `just check` on the new test or committing anything -- source fix, test, and snapshot all uncommitted, no ESCALATION.md. Coordinator did not verify or fix by hand (AGENTS.md rule: never hand-edit a lane, no matter how small); resumed the same session with `opencode run --session <id>` asking it to run `just check` on the new test and commit. | 25 steps then a resume in progress at tick end | No -- the heredoc-to-/tmp rejection is sandbox policy, not opencode-specific; unlike the other four, this worker did NOT give up after the rejection, it just ran out of steps before reaching a commit. |
| 2026-08-30 | issue-116 (jq recursive-enum printer cycles, gh:116), follow-up | The resumed session (above) also ran out of steps: this tick found the lane worker gone again, tree in the identical uncommitted state (no new commits, no ESCALATION.md). Coordinator did not hand-fix. Redispatched fresh via `dispatch-worker.sh` (a new `opencode run`, not another manual `--session` resume) with an explicit "do not start over, just verify and commit" brief. | one more redispatch; still zero commits after two step-budget exhaustions on the same small diff | Unclear -- two stalls in a row on one lane is worth watching; if a third redispatch also fails to commit, treat it as a brief-wording problem (ask for the commit earlier, before the worker's own extra verification) rather than bad luck. |
| 2026-08-30 | issue-133 (euler-pages-restore, gh:133) | Two full runs (16 then 17 steps, $0.03 total), both zero commits, zero file writes -- entirely research (gh issue reads, git log/show of the pre-removal page content, reading tests/euler_real_data.rs and the docs harness). Both ended the same way: it correctly worked out it needed a small synthetic input/output pair to give each restored page a real, committable proof (the issue body's own suggested shape), tried to compute one by writing a scratch `.toy` file under `/tmp/opencode/...`, got `external_directory` permission-rejected, and just stopped -- no attempt to write the scratch file inside the worktree instead, where it had a normal write permission the whole time. | ~$0.03, 33 steps, zero progress twice, coordinator had to identify the actual fix from the event log | Unclear -- distinct from the /tmp-rejection-but-kept-going shape (issue-116): this worker gave up entirely rather than finding the write permission it already had one directory up. Redispatched with an explicit instruction to compute synthetic outputs via a scratch file inside the worktree (`cargo run -- run scratch.toy`), never under `/tmp`. |
| 2026-08-30 | issue-133 (euler-pages-restore, gh:133), third run | The scratch-inside-worktree redispatch (above) died on a DIFFERENT denial: `grep ... \| head \| while read ...` -- shell loop constructs are not in the allow-list (deliberately: opencode cannot inspect a loop body, so allowing `while *` would let anything hide inside one, `git push` included), and the headless auto-reject killed its file-sweep plan. Exited commitless again, ~121s. Root cause now understood: loop-heavy sweep tasks structurally collide with the allow-list. Fix shipped in the brief template (enwiro-delegate skill): loops are named as a KNOWN DENIAL up front, with the two sanctioned alternatives (one file per tool call, or a `python3` script in the worktree -- `python3 *` is allowed). | third zero-commit run on one lane (~$0.04 total across three); ~70 min of lane wall-clock and repeated tick recovery workload | No -- any headless worker under this allow-list hits it; the finding is that the BRIEF, not the list, must carry the sandbox's known edges. |
| 2026-08-30 | issue-133 (euler-pages-restore, gh:133), runs 4-5 | Run 4 (21:07, generic continuation brief from a tick) reached for the identical `grep \| while` pipeline and then died hard enough to take the wrapper with it -- no telemetry row, no exit tick (that gap is now closed: the wrapper fires the tick from an EXIT trap, 59167ed). Run 5 (21:16, brief with the explicit loop ban) obeyed the loop ban but created its scratch .toy via bash redirection, equally denied -- 14 steps, exited commitless. The load-bearing distinction the model kept missing: opencode's write/edit TOOLS are allowed in the worktree, bash file writes are not. Brief template updated to say so in caps; run 6 dispatched with it. | five commitless runs (~$0.06 total) and ~100 min of lane wall-clock on one small docs task | Unclear -- the sandbox edge is model-agnostic, but five consecutive failures to route around it is a DeepSeek-tenacity data point for the 30-lane review; if run 6 fails, escalate this task shape to a stronger OPENCODE_MODEL rather than re-brief again. |
| 2026-08-30 | issue-133 (euler-pages-restore, gh:133), run 6 -- RESOLVED | Run 6 did the actual work (write/edit tools, 317+/99- across five docs pages and the test), verified nothing itself: its verify/cleanup phase used multi-command bash lines (`printf > scratch.toy, ...` and `rm -f scratch.toy, ls, git status`) which the allow-list rejects as compound commands, so it exited without committing. The coordinator ran `just check` in the lane (green), resumed the session (`opencode run --session`), and the worker committed ba9dff7 on instruction; folded to the accumulator same tick. Separately this fold exposed that `bc` is not installed on this host: `changed_lines()` in land-lane.sh/drive-tick.sh/dispatch-worker.sh silently returned 0 for every branch (the `|| echo 0` ate the 127), so SIZE-based auto-promotion had never actually fired -- all three sites now sum with awk. | six runs / ~2h lane wall-clock for one docs task, but the work itself was one run's worth; plus the latent size-check bug found | Two lessons: (1) compound bash lines are their own denial class -- brief template now says one simple command per bash call; (2) a dead-but-dirty lane whose diff passes the gate is salvaged by session-resume-to-commit, not a seventh fresh run. |
| 2026-09-02 | issue-140 (sum-max-reductions, gh:140) | Stuck-lane alarm fired for a lane whose work had already landed:the incident evidence in `plans/incidents/issue-140-20260901/` predates the landing via 3b8c0d7.The lane was not actually stuck;the watchdog raced a landing it had not yet observed,and three commitless re-runs each re-did the same deep-dive instead of noticingthe ancestor relationship. | $0.00, no rework, no coordinator intervention --the landing was real and the re-runs were redundant| No --any watchdog racing a landing would see the same false alarm regardless of worker; not opencode-specific.The lesson is to check ancestry before re-running a flagged lane. |

Four of the five incidents above share one shape: a worker's OWN verification or research step (not its core implementation work) triggers a permission rejection, and the worker either abandons a mostly-finished branch (issue-98, issue-129) or gives up immediately with zero progress (issue-88, issue-108, issue-125) rather than proceeding without the rejected step or leaving an ESCALATION.md explaining why it stopped. The uncommitted-but-good branches were both salvageable by the coordinator; the zero-progress lanes were not (nothing to salvage, only to re-brief). Worth a brief-wording fix at some point: tell workers explicitly that a rejected tool call is not a stop condition -- commit what exists, note the gap, and keep going, the same instruction ESCALATION.md already carries for genuine design questions.

## Speedups shipped with the rollout (2026-08-30, same day)

- **Event-driven landing**: the worker wrapper fires a drive tick on exit; the tick
  gate lands worker-gone + ahead + clean immediately (no 8-minute quiet window --
  that heuristic existed because a claude turn-end was indistinguishable from a
  crash; an opencode exit is unambiguous).
- **Enwiro-free lanes**: `dispatch-worker.sh` = worktree under
  `~/.local/share/toylang-lanes` + background worker; the gh:124 worker pool and
  per-issue enwiro envs are legacy, kept only until their in-flight lanes land.
- **sccache** as RUSTC_WRAPPER in the worker env: cold worktrees share compiled
  crates; landed lane worktrees are removed at landing, so disk stays flat.

## Model ladder candidates for the re-evaluation (maintainer research, 2026-08-30)

The rollout deliberately runs ONE model (DeepSeek V4 Flash) so the 30-lane comparison
stays clean. The maintainer's OpenRouter survey (ZDR-only guardrail active -- routing
is limited to zero-retention providers, DeepInfra/Baseten being the reliable coding
route) named the candidates for a post-review tier ladder:

- **GLM 5.2** (~$0.49-0.76 in / $1.56-2.42 out per 1M, 1M context, built for
  project-level software engineering and long-horizon agent work): the candidate for
  research dispatches and design-heavy lanes -- an order cheaper than sonnet, an order
  stronger than flash. `OPENCODE_MODEL` already carries per-dispatch overrides.
- **Ling-3.0-flash** (~$0.06/$0.18, 5.1B active params, single ZDR provider at ~96%
  uptime): possible ultra-cheap tier for trivial mechanical rows; an optimization,
  never a dependency.
- Skip per the same survey: Kimi K3 (half of Opus price, not Opus class), MiMo-V2.5
  and Qwen3.7 Flash (unreachable or too slow under ZDR).
- The radical option the review should price out: the COORDINATOR tick itself on GLM
  5.2 via opencode. Sonnet ticks are now the dominant cost of the whole loop; the
  blocker is that the tick contract is deep in claude-code machinery (skills, resume,
  stream-json, hooks), so this is a real port, not a model swap.

## Baseline, for the eventual comparison

- Trial lane (gh:114, 2026-08-30, pre-rollout): mid-tier emit_llvm refactor, landed
  end to end, $0.04 total, zero review findings, one self-corrected compile error.
- Known limitations going in: no server-side classifier (mitigated by the deny-by-default
  allow-list config in the maintainer's chezmoi, never `--auto`); no container/egress
  isolation yet (published best practice for unattended runs; accepted for this
  self-authored public repo); worker cannot receive SendMessage nudges (steer by
  killing + `opencode run --session <id>` resume with a new message).

## Incident: repeated N=150-for-157 dispatch mistake, and a stale board note that made it worse (2026-08-31)

A tick dispatched gh:157's brief with `dispatch-worker.sh 150` instead of `157`,
landing in the just-freed issue-150 worktree. A concurrent/duplicate tick session
(same underlying session id, two `claude --resume` processes observed running at
once -- see the concurrent-sessions memory note) caught this independently and
committed a board note (fb7754d) marking the row `delegated` with a "worker is
live in toylang-lanes/issue-150" warning and a follow-up to rename that branch to
issue-157 once the worker exited.

That note was wrong on the facts: the issue-150 worktree only ever held issue-150's
own already-landed let-bindings/input-annotation work (folded into
to-merge-1788204752 this session); no gh:157-related worker ever actually ran there
-- both the original mistaken dispatch and its bash wrapper appear to have been
killed by session teardown (background job, "[killed]", zero output, task
notification "no completion record found") before `dispatch-worker.sh` got far
enough to print anything. Following the note's instruction (rename that branch to
issue-157) would have mislabeled unrelated, already-consumed work.

Fix applied: cleared the note, reset the row to `status: todo`, dispatched
`dispatch-worker.sh 157` correctly into the actual free lane.

Lesson: a board note written under time pressure to prevent a duplicate dispatch
should describe the *risk*, not assert unverified internal state ("worker is live")
as fact -- the next tick then has to re-derive ground truth from disk anyway. Cheaper
to just mark `status: blocked-pending-verification` and let the next tick check
`git log`/`pgrep` itself.

## Incident: erlang-target-research (gh:163) gave up on a toolchain-check denial (2026-09-01)

18 steps of legitimate research (read `docs/reference/types/{stream,str,char}.md`,
`research-log/index.md`, `ESCALATION.md`, grepped `Backend::` in `src/lib.rs`, read
backend emitters) then tried `which erl escript erlc; erl -eval '...halt().' -noshell`
to check whether an Erlang toolchain exists on the host -- denied by the permission
classifier -- and exited immediately after, zero commits, no `plans/*.md` written, no
ESCALATION.md for this run. Landed on a lane whose branch tip happened to equal the
`to-merge-1788204752` accumulator (its dispatch base, since that accumulator was the
largest live one at dispatch time) -- misread by the gate script's `ahead=7 dirty=0`
signal as "worker exited, landable," when actually zero commits were this worker's own.

Root cause: the task never needed to actually run Erlang. The brief asks for a design
survey (process/effect model, pattern matching, immutability) against docs already in
the repo and toylang's own backend source -- a desk review, not an empirical one. The
worker chose to verify toolchain presence anyway, hit the sandbox wall, and gave up
rather than continuing with the read-only research it had already started.

Fix: rebriefed with an explicit "no toolchain/execution needed, docs+source read only,
write findings to plans/erlang-target-research.md" instruction. | $0.01, 18 steps,
zero commits, one coordinator rebrief | No -- the `which`/`erl -eval` denial is sandbox
policy (arbitrary binary execution), not opencode-specific; the give-up-after-one-denial
behavior is the same shape as issue-108/125 above.

## Ruling: escalation/decision tracking moves off GitHub issues (2026-09-01)

Maintainer answer to the coordinator-escalation-brief-phrasing-experiment round
(inbox record, 2026-09-01T20:36:51Z): "we can just have a local inbox for such
cases, no need to rely on github issues; i guess actually we can migrate the
whole flow to work entirely on files committed into the repo at this stage."

Immediate action taken: the blocked note (brief-phrasing-experiment) got a board
row with no `issue:` field instead of a `gh issue create` retry -- board.yaml
already supports issue-less rows, so this needed no new mechanism.

Open, not yet scoped: the maintainer's broader remark reads as a ruling that the
whole escalation/decision-tracking flow (currently: compose note -> file gh
issue -> board row references it) should move to files committed in the repo
instead of GitHub issues. This is a process/tooling change bigger than one note
and hasn't been designed yet -- needs a decide row (what replaces "issue:
gh:N" as the cross-reference key; how history/discussion attach to a row
without an issue thread) before any dispatch. Not board-added yet; flagging
here so the next tick that touches escalation composition sees it before
defaulting back to gh issue create.

## Incident: issue-154 stuck at 3 commitless runs on a self-inflicted denial loop (2026-09-01)

Root cause (from `20260901-221717-issue-154.jsonl`): the worker built real work
(input-type-annotation + tail-pipe |>, 173 insertions across 4 files, own
ESCALATION.md justifying the scope), then verified it by writing
`scratch_tailpipe.toy` and repeatedly running `cargo run -q -- run
scratch_tailpipe.toy` -- direct binary execution, always denied. The run ended
mid-loop on that exact denial (`UnknownError: The user rejected permission to
use this specific tool call`), with nothing ever committed. Same failure shape
as issue-93 and issue-155's "permission denial on direct binary execution".

Fix applied: added `cargo run -- run <file>` / direct binary execution to the
KNOWN DENIALS boilerplate in `dispatch-worker.sh`, pointing workers at
`tests/corpus/*.yaml` + `just check` (AGENTS.md's sanctioned verification path)
instead of scratch-file-plus-manual-run. Rebriefed issue-154 to keep its
existing uncommitted work, convert the scratch file to a corpus case, and
commit once `just check` is green.

## Incident: issue-149 stuck at 2 commitless runs, all-or-nothing scope on a 6-backend task (2026-09-02)

Root cause (from `20260830-220941-issue-149.jsonl` and `20260901-232217-issue-149.jsonl`):
both runs treated "Implement Float across the backends" (gh:149, board row
float-build) as a research task and spent their entire step budget writing and
running cross-language probe scripts (Go, Lua, C via `cc`, jq, Node, Python) to
characterize each backend's native double-to-string formatting, then hit the
step limit with zero commits both times. This is legitimate groundwork (per
plans/questions.md#q37, printing format is explicitly unruled per-backend
conformance work) but the runs never got past survey mode into writing the
actual formatter code, and held all 6 backends' work uncommitted while
chasing full completion instead of landing backends incrementally.

Diagnosed directly rather than dispatching an investigation worker (the
evidence was conclusive from the logs alone). Rebriefed issue-149 in place:
pointed at the existing probe output instead of re-deriving it, named ADR
0007 + JS's Number.prototype.toString() as the reference algorithm, and
required committing one backend's formatter at a time rather than holding
everything uncommitted until all six are done.

## issue-158 stuck lane: already explained, no dispatch needed (2026-09-02)

`stuck-issue-158-investigation` asked to diagnose why issue-158 (shell-out-build,
gh:158) had two commitless runs. Already answered on the board itself:
`shell-out-build` carries `needs: [stdout-stderr-effect-model-design]`, and that
decide row already has a composed, pending grill round
(docs/.grill/stdout-stderr-effect-model.round.yaml) asking exactly the Q35
question both runs got stuck on. Archived the investigation row instead of
dispatching a worker to re-derive an answer that's already on file -- the real
next step is the maintainer answering the pending round, not more automated
investigation.

## Incident: issue-168 investigation dispatch used the wrong brief wrapper (2026-09-02)

Dispatched `stuck-issue-168-investigation` via `dispatch-worker.sh 168 "<investigation
brief>"` without `BRIEF_RAW=1`. The script's default wrapper always prepends "Your
task is GitHub issue #$N: run `gh issue view $N` ... $BRIEF" -- for issue-168 that's
the *original* feature issue, not "investigate why this lane is stuck." The live log
confirms the worker read gh:168's real issue text and started exploring the actual
offload/vectorizability feature (reading tir.rs, draft.md's offload section, corpus
tests) instead of the frozen incident evidence at
plans/incidents/issue-168-20260902/. Attempted to kill the misdirected process and
was blocked by the auto-mode classifier (process-kill outside the sanctioned
scripts) -- per house rule, not retrying through another channel. Letting this run
finish; it will likely land as another commitless run since the worker is doing
the wrong task. Next tick should redispatch issue-168's investigation with
`BRIEF_RAW=1` (the script's own doc comment already names this as the case
"research dispatches, custom continuations" need), and this note stands as a
reminder for every future stuck-lane investigation dispatch: always set
`BRIEF_RAW=1`, never let the standard build-brief wrapper attach to an
investigation task.

## Incident: land-lane.sh's auto-commit only covers tracked files -- untracked
## research findings are at risk of being cleaned away (2026-09-02)

Found four orphaned research lanes (issue-trait-interface-research,
issue-signature-matching-deeper-research, issue-http-query-sugar-research,
issue-recursive-descent-order-research) with no activity since 2026-09-01
~21:30, invisible to the current stuck-lane snapshot (their names don't match
the numeric `issue-<N>` pattern the tracker expects). Two of them
(trait-interface-research, signature-matching-deeper-research) have real
findings files sitting **untracked** (`?? plans/<name>.md`) -- never `git
add`ed, so they are not "tracked dirty" and the 2026-09-02 auto-commit ruling
in land-lane.sh (`git -C "$d" add -u` then commit) does not pick them up.
Worse: the very next step in `land land-lane.sh` runs `git clean -fdq` on any
remaining untracked files as "sanctioned scratch" -- if this were run against
these lanes as-is, it would silently **delete** the findings before they were
ever persisted. Did not run `land-lane.sh land` on these two lanes to avoid
that. Dispatched continuation workers instead (BRIEF_RAW=1) whose only job is
`git add` + `git commit` for the existing findings file, so the tracked-dirty
path lands safely on the next `land-lane.sh` pass. The other two lanes
(http-query-sugar-research, recursive-descent-order-research) have zero diff
from main -- they produced nothing and need re-investigation, not landing.
Follow-up: land-lane.sh's auto-commit should `git add -A` (not `-u`) when the
gate is green, or the cleanup step should skip files that look like research
output (`plans/*.md`) -- filed as a board row.


## Investigation: stuck lane issue-170 (gh:170, erlang-toolchain-empirical-research) (2026-09-01

Lane stats at capture: 1 run, 0 commits, clean tree, ahead  ​0, dead 22h. The
event-log tail ends after a completed tool call with no final message, no commit,
no write -- consistent with the step-budget exhaustion already seen on issue-116/133,
though not provable from the tail alone. The branch tip is `d9896ec` (the module-routing
spike commit), so no lane work ever happened on top of the dispatch base.

What the single run actually did (from the jsonl tail): read the issue body via
`gh issue view 170`; failed to find `plans/erlang-target-research.md` in the worktree
(it is absent from this lane base -- it landed on main via the to-merge accumulator
only after the base was cut),and recovered it via `git show 9b9cec3:...` from git
history;read the desk research;ran `which erl erlc escript erl_call` -- ALLOWED by the
permission gate, found nothing on PATH;(in run 1, the compound `which ...; erl
-eval ...` was denied, so the classifier's edge is the compound form, not `which`
itself)ran `erl -noshell -eval ...` -- DENIED ("The user rejected permission to use
this specific tool call");verified `/usr/bin`, `/usr/local/bin`, `/opt`, `/snap` and
`/usr/lib/erlang` -- all empty;and stopped, uncommitted, with the last reasoning
still mid-plan ("let me confirm by attempting to run erl directly").

Diagnosis: this was primarily a **brief-clarity** failure, compounded by task shape
and an asymmetric permission allow-list;,not a capability gap:

- The brief's decision tree had two branches: denial -> document-and-stop; not-denied
  + available -> verify empirically. The observed case -- not denied (the `which`
  succeeded) but also not available -- had no branch. The worker's own reasoning
  identified "a third case: allowed but not present" and then had no scripted
  done-state to land on.
- The brief described run 1's denial as "`which erl` / `erl -eval`" was denied,;
  collapsing a compound-command denial into two individual denials. When `which`
  alone succeeded, the worker concluded the denial case was off the table and pursued
  running `erl` anyway -- exactly the outcome the denial branch existed to prevent. The
  actual classifier edges: compound shell forms auto-reject, executing a non-whitelisted
  binary is denied, `which` alone is fine.
- The brief referenced "plans/opencode-rollout.md, incident 2026-09-01" -- but this
  lane's copy of that file has no such entry (the branch predates the incident-log
  commit),and the addendum target `plans/erlang-target-research.md` is absent from the
  lane base. The worker recovered both from git history, at the cost of several steps
  and early confusion.

- Task shape: the core ask -- "actually install/verify the Erlang toolchain and run the
  documented-semantics comparison against real erl/erlc output" -- is impossible on
  this host: no Erlang binaries exist (PATH + five filesystem locations verified empty),running
  `erl` is denied by the permission allow-list;,and installation (apt/snap/network or
  executing an installer) would almost certainly hit the same walls (unverified;not
  attempted). The only deliverable available is the negative finding, which the brief
  half-defined.
- The permission system is an aggravator, not the root: its per-command behavior is
  stable but undiscussed at the granularity the brief needed, and the deny message "The user
  rejected permission" doesn't name the deny system, so a worker cannot tell policy from
  a human intervention.
- Not a capability gap: the worker read the missing file from git history, named the
  unscripted case explicitly, adapted after the `erl` denial (via `ls` checks, not a second
  execution channel),and ran far past where run 1 gave up. Nothing in its reasoning
  suggests it couldn't have written the addendum;the failure is that no instruction told
  it the absence was itself the deliverable, plus a likely step-budget exhaustion.

Recommendation:

- **Reshape gh:170's row** from "empirically verify Erlang" to "determine whether an
  Erlang toolchain can be exercised here, and record the finding". The brief must enumerate
  the full outcome space: (a) toolchain found -> run the comparison script against real
  erl, append results;(b) toolchain absent -> append an addendum to
  plans/erlang-target-research.md documenting: no binaries on PATH or in /usr/bin,,
  /usr/local/bin, /opt, /snap, /usr/lib/erlang;executing erl is additionally denied
  by the allow-list (observed: "The user rejected permission");installation was not
  attempted and is presumed blocked the same way (flag as unverified;;and stop -- that
  addendum is the deliverable, not a failure.(c) any check denied -> document exactly what
  was denied and stop.
- **Then close gh:170**:the empirical comparison cannot happen here, the desk research
  (plans/erlang-target-research.md) remains the deliverable for gh:163,and surface the
  maintainer's flagged concurrency open item as its own board row now -- the issue body's
  "once this lands" condition has effectively become "cannot land here"..
- **Dispatch-template fixes**, generalizable: (1) any "verify tool X" brief must enumerate
  all three outcomes (present / absent / denied)and define a done-state for each -- "a tool
  that does not exist is a complete finding, not a stop condition", the mirror of the existing
  "a rejected tool call is not a stop condition" rule;(2) describe the permission system's
  edges as command classes, not as the compound commands that triggered them;(3) when a
  brief references a file that may not exist on the lane base, either cut the lane from a base
  that contains it or give the worker a `git show` pointer;(4) the step-budget exhaustion
  has now hit three lanes with zero commits each (issue-116, issue-133, this one)--
  worth instructing workers to write and commit partial findings before further verification,
  or raising the budget;(5) for the 30-lane review: this lane did NOT reproduce run 1's
  give-up-on-first-denial shape -- the worker pushed through multiple denials and checks -- but
  landed in the same commitless zero-output state, because it ran out of scripted outcomes

  before it ran out of steps. The bottleneck is the decision tree, not tenacity.

## Investigation: stuck lane issue-float-build-python (2026-09-04)

Verdict: **tooling/permission trap**, compounded by a brief-wording gap
and a content-channel corruption, not a capability gap. The lane's one run
produced real, committable work -- the entire Python float emitter diff
(src/emit_py.rs, +71/-3, ASCII-clean) -- but died at a denied scratch-probe
execution with nothing committed, no tests written, no `just check` run, no
ESCALATION.md. Lane stats at capture: 1 run,  0 commits, tracked-dirty
tree, dead ~38h (evidence fetched from git via ca1f011, since the lane checkout
predates the freeze note).

What the run did: read the JS reference tests, implemented FLOAT_HELPER
(Python repr reshaped into JS's String(number) spelling: exponent expansion,
`.0` strip, exponent unpadding),, DIVF_HELPER (IEEE zero-divisor answers through
tl_divf),,, wired show()/expr()/arith() and the helper gating,, then cleaned the file
to plain ASCII. It then planned backend_py.rs tests (mirroring the JS five plus
exponent-boundary cases),, got worried whether Rust's `f64::to_string()`
(`src/float.rs::lit` is `n.to_string()`) might emit a plain-decimal literal Python
would parse as an int (a real hazard: tl_float's reshaping only fires when repr
has an `e` or `.0`), wrote f64_probe.rs, compiled it with `rustc` (allowed),
tried `./f64_probe` -- denied (direct binary execution), and ended right there.



Two aggravators made the run as expensive as it was. First, the worker's own
tool-call text got corrupted in transit, intermittently: combining accents inserted before digits, `)` -> `[]`, `,` -> `[],`, `the`/`ther` swaps,
dropped parens -- across write/edit/bash-c strings. It diagnosed this forensically
(hexdumps, sub-string probes,)and adapted (single-line edits,, "no risky
characters", a `python3` script for the comment surgery the edit tool couldn't
match), but the forensics consumed roughly the majority of the run's steps. This
is an environment problem, not a capability failure -- and worth a landing-time
scan: this lane's diff is clean only because the worker ran its own byte scan;a
lane that skipped that would have committed the corruption silently. Second, the
fatal probe question (Rust's f64 Display spelling at exponent boundaries)has no
sanctioned direct answer on this host;the sanctioned routes were the test suite
(write the tests, `just check`, read failures),the already-board-planned
float-format-research, or neutralizing the hazard outright (`tl_float` coercing
`float(n)`, or `lit` emitting a Python-float spelling). The worker's instinct was
sound, the route wasn't briefed.



Diagnosis, on the four categories:

- **Tooling/permission trap** (fatal step):`./f64_probe` is direct binary
  execution, a KNOWN denial class named in the brief by toylang-runner examples
  only (`cargo run -- run file.toy`, `./target/debug/toylang`),which the worker
  didn't generalize to its own rustc-compiled binary. `rustc f64_probe.rs
  -o f64_probe` compiled fine,so the classifier allowed the compile and denied the
  run -- a half-sanctioned dead end.

- **Brief clarity**:the denial list names the class by example, not by
  principle,so "a binary I compiled myself" didn't look covered;the commit-early
  rule was present but aspirational ("never let cleanup failures stop you
  committing"),not an ordering,so real work sat uncommitted through verification.

- **Not a capability gap**:the reasoning is sharp throughout -- correctly
  identified the int-absorption hazard, planned the right tests, diagnosed and
  routed around the corruption,, produced exactly the emitter shape the JS reference
  calls for. It hit the wall on an unsanctioned verification step after the work was
  done,, with the step budget spent.

- **Task shape**:the underlying formatting question is the family-wide snag the
  float-format-research escalation already names -- Go/Python/Rust siblings stalled
  6 cumulative runs probing target float formatting before any formatter code.

  This lane got furthest (real emitter diff);the research row, already dispatched
  and pointed at this lane's probes, is the right unblock



Recommendation:

- **Reshape the immediate action to a session resume, not a redispatch**:keep
  the uncommitted emitter diff,and brief via `opencode run --session <id>`:(1) commit
  the emitter diff FIRST, as its own commit, before any further verification;(2)
  then add tests/backend_py.rs Float tests mirroring backend_js.rs plus exponent-
  boundary cases;(3) verify with `just check` only -- failure output is the empirical
  answer it was probing for;(4) never compile-and-run a scratch probe -- `rustc -o` +
  `./binary` is the same denial class as `cargo run`,and `rustc <file>.rs -o ...`
  should be treated as denied outright;(5) if a test surfaces the int-absorption
  hazard, fix it in `tl_float` by coercing `float(n)` up front (one line;makes
  Rust's Display spelling irrelevant). One backend, one or two commits, land promptly.



- **Sequence behind float-format-research**:this lane's diff (read via `git -C
  ~/.local/share/toylang-lanes/issue-float-build-python diff`)is that row's best
  concrete lead;the research brief should add the int-absorption edge as the specific
  snag this lane died on,and Rust's `Display` (`src/float.rs::lit`)as the emission
  path to characterize.

- **Dispatch-template fixes**, generalizable:(1) name the direct-binary-execution
  denial by principle -- "any binary you compiled, however you compiled it (rustc
  -o, cc, go build), is direct binary execution, never run it" -- and add
  `rustc <file>.rs -o ...` to the KNOWN DENIALS outright;(2) make commit-early a hard
  ordering -- "commit each piece of real work as you finish it, verification happens
  after commits, not before" -- the current phrasing has now failed the same way on
  this lane's siblings (issue-98/129/133-run6/154/170);(3) scan landed lane diffs
  for the corruption's tells (non-ASCII, U+0301, `[]` for `),`, `ther` for `the`),,
  since a lane that doesn't run forensics commits it silently.



- **Escalation threshold**:if the resumed run also fails to commit on this lane,
  escalate to a stronger OPENCODE_MODEL -- six commitless runs across the float-build
  sibling family already argue for research-row-first sequencing,not another probe-happy redispatch of the same model.
  before it ran out of steps. The bottleneck is the decision tree, not tenacity..

## Investigation: stuck lane issue-float-build-rust (gh:149, Float for the Rust backend)(2026-09-04

Lane stats at capture (frozen in plans/incidents/issue-float-build-rust-20260904/: 1 run, 0 commits, clean tree, ahead 0, dead since 2026-09-02T19:20Z, evidence captured 2026-09-04T09:44Z). The board row is still `status: todo` and the branch tip equals the dispatch base (35f0748, itself on main), so zero lane work ever happened. The event log shows a single ~2-minute run ending mid-step with no final message, no writes, no commits.

What the single run did (from the jsonl tail): read AGENTS.md, listed just recipes;failed to view the issue as `gh issue view float-build-rust` -- that's a lane name, not an issue number, and recovered gh:149 from board.yaml;read gh:149's body;then spent the bulk of its budget re-deriving the JS reference from git history: multiple `git show f22a806 -- <paths>` calls (each re-printing the whole commit message and stat), full reads of emit_js.rs and emit_rs.rs from that old commit instead of the live worktree files, and reads of shared files from the same commit. By the end its reasoning already carried the full implementation plan:the four `unreachable!` Float arms in src/emit_rs.rs (lines 907, 936,1176,1433) to fill, a `tl_parse_f64` parser modeled on `tl_parse_i32`, Rust's f64 Display printing `inf`/`-inf` where JS prints `Infinity`/`-Infinity`, f64 arithmetic having no `wrapping_*` and total division by zero, tests going in tests/backend_rust.rs rather than corpus. Then it tried to read `tests/backend_rs.rs` (no such file -- it is `backend_rust.rs`), listed tests/, grep'd `Backend::Rs` (no matches -- the variant is `Backend::Rust`),and died at the start of the next step. Zero tool calls were denied all session -- the "invalid issue format" and "File not found"/"No files found" are ordinary errors, not permission rejections.



Diagnosis: primarily **task shape**, compounded by **brief clarity** -- not a capability gap, not a tooling/permission trap:

- The natural approach to this row -- read the whole JS reference, the whole Rust emitter, the test harness, then write -- blows past a one-run budget before the first write, exactly the shape that killed issue-149 and issue-170. The worker gathered the reference from the landing commit piecewise instead of from the live worktree files (src/emit_js.rs already implements Float in HEAD: the Float arms are at emit_js.rs lines 391, 775,1056;the four unreachable arms at emit_rs.rs lines 907,936,1176,1433 are the entire surface to change),which multiplied the context load and never reached the write phase.

- The brief's issue reference was unusable:`gh issue view float-build-rust` cannot work (lane name is not an issue number;;three calls were spent recovering gh:149 from board.yaml. The brief also didn't name the test harness filename (backend_rust.rs, not backend_rs.rs)and didn't instruct early commit -- both known failure modes from issue-170's recommendations。
- Not a capability gap:the run's own reasoning had the full, correct implementation designed before it died. Nothing in the transcript suggests it couldn't have written the diff;it simply never got the chance.



Recommendation:**re-dispatch the same row**, not a drop and not a scope reshape --the work is real, bounded(one emitter diff + a parser helper + tests mirroring tests/backend_js.rs),and the JS reference is now live in the worktree, so nothing forces re-deriving it from git history. The rebrief should:

- Name the task by board row and issue number (float-build-rust, gh:149)and skip the issue body entirely --the live JS emitter is the spec. Point at the live files:the Float arms in src/emit_js.rs are the reference, and the four `unreachable!` Float arms in src/emit_rs.rs (plus a `tl_parse_f64` next to `tl_parse_i32` in the PARSER_HELPER) are the entire surface to change. No `git show` of old commits needed。
- Name the test harness exactly:`tests/backend_rust.rs`, `Backend::Rust`;mirror the Float tests from the JS commit into tests/backend_js.rs (no corpus cases --the other backends still unreachable-arm Float)。
- Pre-state the gotchas the worker already derived, so it doesn't re-burn budget on them:Rust's f64 Display prints `inf`/`-inf` (map to `Infinity`/`-Infinity`;NaN already matches);f64 division by zero is total (returns Infinity, IEEE;, no guard needed,and there are no `wrapping_*` methods on f64;the checker already accepts Float input (per the JS commit)。

- Commit early:write the emit_rs.rs diff first and commit it (the row's own one-backend-one-commit contract),then add tests,then `just check`. A partial commit beats another zero-commit run;;if the budget runs low, commit what exists and note the gap, per the standing "a rejected tool call is not a stop condition" rule。

- Mark the row `delegated` on re-dispatch (it is still `todo`, so another tick could double-dispatch),and rebase the worktree onto current main before dispatching (the lane base is 88 commits behind origin/main;the task is self-contained against files already in the base, so rebasing is cheap insurance against a land-time conflict)。

Written by DeepSeek V4 Flash via opencode.
| 2026-09-02 | issue-169 (stuck-issue-169-investigation, gh:169) | The stuck-lane investigation and the escalated original task (issue-150 let-bindings, later re-scoped to an `input <type>` annotation per the lane's own committed `ESCALATION.md`, 4df542d) share one lane/worktree. After that escalation was ruled on and a continuation was dispatched, two more runs fired in the same worktree (~21:03-21:05) and both ended the identical way: read `ESCALATION.md` and sibling incident folders (hitting permission denials on cross-worktree reads), wrote nothing, then auto-fired landing with zero commits ahead of main. Four consecutive commitless runs total on this lane (2 pre-escalation, 2 post-rebrief). | $0.005, 4 commitless runs across two dispatch cycles, zero code written | Unclear -- the shared-lane design (one worktree serving both the meta-investigation brief and the escalated feature's continuation brief) looks like the real defect: whichever brief a generic continuation dispatch resumes is ambiguous, not a DeepSeek-specific failure. Escalated to the maintainer (issue-169-investigation-stall round) rather than redispatching a fifth time. |

## Stale board row: `input-type-annotation-build` (gh:150) had already landed (2026-09-02)

Tried to dispatch the freshly-boarded `input-type-annotation-build` row (issue: gh:150) into
a lane and `dispatch-worker.sh` refused: branch `issue-150` already exists with no worktree.
`git merge-base main issue-150` came back equal to `issue-150`'s own tip (`421450f`, dated
Aug 30) -- that commit is already an ancestor of `main`. It IS the `input <type>` annotation
this board row asked for (same corpus fixture, `tests/corpus/input_annotation.yaml`, same
`{x, y}` shape), landed by an earlier lane before the issue-169 shared-lane saga even started.
`just check` is green on current `main` with the feature present. Archived the row
(`board-archive.py input-type-annotation-build`) instead of dispatching a duplicate worker;
did not touch the orphaned `issue-150` branch (deletion isn't this router's call). Lesson:
before dispatching a freshly-boarded build row, check whether an orphaned branch of the same
name already contains it -- `dispatch-worker.sh`'s stale-branch refusal is a real signal to
inspect, not just a naming collision to route around.

## Near-miss: `stuck-issue-172-investigation` dispatched without `BRIEF_RAW=1`, reused a live gh number (2026-09-02)

Same class of mistake as the issue-168 incident above (2026-09-02, "used the wrong brief
wrapper"): dispatched `dispatch-worker.sh 172 "<investigation brief>"` without `BRIEF_RAW=1`
for a lane whose number (172) is also a real, unrelated open GitHub issue (the gh:159 re-file,
stdin/`Stream<Str>` redesign) -- the standard wrapper's `gh issue view 172` pulls that issue's
real text, not investigation instructions. Unlike issue-168, the worker's live log shows it
followed the task-specific investigation text anyway (went straight for
`plans/incidents/issue-172-20260902/`, pulled the frozen evidence via `git show
e7a290d:plans/...` when the local copy was missing) rather than getting misdirected into the
stdin-redesign feature -- so this run looks fine in progress, but it was luck, not the brief
being correct. Also: board row `stdin-redesign-build-2` (status: delegated) targets the same
lane number (issue-172) for the *actual* gh:172 feature work; if that row's own dispatch
follows later, it will land in the same worktree as this investigation, the exact shared-lane
shape that stalled issue-169. Follow-up for next tick: always pass `BRIEF_RAW=1` for
stuck-lane-investigation dispatches (the script's own doc comment already names this as the
"research dispatches, custom continuations" case), and give `stdin-redesign-build-2` a lane
number that doesn't collide with an investigation row before dispatching it.
| 2026-09-02 | issue-153 (stuck-issue-153-investigation / declare-terminator-build, gh:153) | Runs 1-2 died from backtick-command-substitution permission denials, rebriefed with that root cause on 2026-09-01 (3a2d02d). Runs 3-4, dispatched with the corrected brief, hit a different wall: edit-tool string-mismatch failures partway through the same `src/parse.rs` refactor (tokenize `;`, remove the old cross-line-call heuristic, rewire `input <type>`), leaving a coherent but uncommitted diff each time -- `Tok::Semicolon` is tokenized but never consumed; no run reached the actual terminator-parsing change gh:153 asks for. | $0 marginal (all four runs zero-committed), 4 commitless runs across two dispatch cycles, zero code landed | Unclear -- the diff's shape suggests dispatch size/duration, not task difficulty: one continuous session tries to tokenize + delete a heuristic + rewire a call site + add new parsing all at once. Escalated to the maintainer (issue-153-investigation-stall round, options: split into two smaller dispatches / stronger model / drop) rather than redispatching a fifth time. |

## Resolved without a third dispatch: `stuck-issue-172-investigation`, root cause fully visible on disk (2026-09-02)

2 commitless runs, both under lane `issue-172` (the near-miss above already flagged this
lane collided with `stdin-redesign-build-2`'s real gh:172 work). Read both event logs
directly instead of dispatching a third run:

- Run 1 (`20260902-204336-issue-172.jsonl`): spent its whole budget re-deriving context
  (`gh issue view 172`, walking `issue-159`'s abandoned worktree, `git show` on the
  reference diff) then died when the user-permission layer rejected a `read` of
  `/tmp/ref.diff` -- never reached the incident evidence or wrote a report.
- Run 2 (`20260902-214450-issue-172.jsonl`): correctly found the frozen evidence at
  `plans/incidents/issue-172-20260902/` via `git show e7a290d:plans/...` (local copy was
  missing, main was 6 commits ahead of the checkout), read `opencode-rollout.md` for the
  report format, then died the same way -- a `cat` of the full first-run log (after already
  reading it once at `limit=`) got rejected by the permission layer. Never wrote a report.

Root cause: both runs are permission-trap deaths, not task-shape or brief-clarity failures --
the investigation *brief itself* is fine (run 2 followed it correctly end-to-end up to the
report step); what killed both runs was re-reading an already-large file a second time in one
shot instead of paging with `limit=`/`tail`. That is exactly the class of thing a fresh
dispatch would repeat, since the trap is in how these workers read logs, not in what they were
told to investigate.

Answering the investigation's own three questions from this evidence directly (no third run
needed): not brief clarity (run 2 read the brief and evidence correctly), not a capability gap,
not task shape -- it is a tooling/permission trap (oversized single-shot reads of files already
read once) compounded by the still-open lane-collision risk with `stdin-redesign-build-2`
(follow-up already logged above, unchanged: give that row its own non-colliding lane before
dispatching it). Archiving `stuck-issue-172-investigation` on this finding rather than spending
a third commitless run to rediscover it.

## `mutation-semantics-spike` and `float-build-lua`: 2 commitless runs each, exploration without a stopping point (2026-09-02)

Both lanes were redispatched once already this evening (~22:14-22:16, corrected at ~22:34-22:39)
and both still landed at zero commits, worktrees exactly at main -- not a permission trap this
time, a different root cause each:

- `mutation-semantics-spike`: the second run did substantial legitimate exploration (linearity.rs,
  tir.rs, emit_lua.rs, emit_rs.rs, ty.rs, corpus tests, draft.md, matcher-parser-spike.md) but
  the brief ("spike the analysis... before a real decide row reopens this") names no concrete
  deliverable, so the worker never reaches a natural point to stop investigating and write.
  It ran to a rejected tool call near the end of budget having written nothing.
- `float-build-lua`: the second run opened with exactly the right reference (`src/emit_js.rs`'s
  Float impl, `src/emit_lua.rs`'s current state) in its first four steps, then abandoned that
  path to spend the rest of the budget diffing `float-build-go`/`float-build-python`'s commit
  history instead -- an unrequested detour into sibling lanes -- and never touched `emit_lua.rs`.

Rebriefed both (BRIEF_RAW=1, continuation dispatch in the same worktree) rather than repeating
the failed brief: `mutation-semantics-spike` now gets a capped exploration budget and a named
three-question findings-doc deliverable to commit even if partial; `float-build-lua` is told
explicitly not to read the Go/Python/Rust sibling lanes and to port straight from the JS Float
impl it already found. Both still under the 4-run escalation threshold.

## `stuck-issue-159-investigation`: root cause already on the board, no dispatch needed (2026-09-03)

`stuck-watch.py` auto-filed this row against the `issue-159` worktree (no activity 4h, 4
run(s), 0 commits at detection time). No worker was dispatched to investigate it: the root
cause is already fully documented elsewhere on the board and predates the watchdog's alert.
`stdin-redesign-build-2` (gh:172)'s own title records the history -- the maintainer ruled
2026-09-02 (option C, "drop the poisoned issue-159 lane, re-board under a fresh lane id rather
than repair it") after the lane collided with a real, unrelated open issue also numbered 172.
The `issue-159` worktree has stood abandoned in place since that ruling, permission-denied
cleanup left as garbage on purpose, its one commit (`45c76be`) kept only as reference for the
re-derived `stdin-redesign-build-2` work.

Answering the investigation's own three questions from that existing record: not brief clarity,
not a capability gap, not a tooling trap -- this was a maintainer cleanup decision, already
executed, that the watchdog has no way to see (it only sees worktree inactivity, not board
history). Archiving `stuck-issue-159-investigation` on this finding; no rebrief or reshape
needed since there is no live task left in that lane to rebrief.

## Resolved without a third dispatch: `stuck-issue-174-investigation`, structurally undoable by a sandboxed worker (2026-09-03)

The original `trait-interface-build` stall (run 1, `20260902-221014-issue-174.jsonl`) is a
task-shape failure: the worker spent its whole ~34-minute budget on broad orientation (full
reads of `parse.rs`, `check/mod.rs`, `ty.rs`, `tir.rs`, `prelude.rs`, `lib.rs`,
`check/types.rs`, plus `draft.md` and grep sweeps for colon-call precedent) across a task that
spans parser + AST + checker + TIR + six codegen backends + prelude impls in one shot, and
never reached a first edit.

The investigation dispatched to explain that (`20260902-224729-issue-174.jsonl`, 7 steps) could
not do its job at all: the incident evidence it was told to read lives at the absolute path
`/home/kantord/repos/toylang/plans/incidents/issue-174-20260902/`, outside the worker's own
`~/.local/share/toylang-lanes/issue-174` worktree/sandbox. Both attempts to read it (the
directory listing and the marker file) were permission-rejected, and the run gave up after 7
steps having written nothing to `plans/opencode-rollout.md`. A second dispatch would fail
identically -- opencode workers cannot read outside their own worktree, so an incident frozen
in the main checkout is structurally unreachable to them, exactly as already established for
`stuck-issue-172-investigation` above.

Answering the investigation's own three questions from the coordinator-side evidence directly:
not brief clarity, not a capability gap in the model itself -- it is a tooling/permission trap
(sandbox boundary) for the investigation row, and separately a task-shape problem (too large
for one-shot orientation) for the underlying `trait-interface-build` row. Archiving
`stuck-issue-174-investigation` on this finding; `trait-interface-build` was rebriefed in the
same tick to a parse-only first slice (AST + parser only, no checker/codegen/prelude), reusing
the freed `issue-174` lane.

## Escalated: three lanes stuck at 4+ commitless runs, same root shape (2026-09-03)

`function-signature-matching-syntax` (gh:152, 4 runs), `stdin-redesign-build-2` (gh:172, 4
runs), and `float-format-research` (gh:149, 6 runs) all independently hit the identical
failure shape: every run reads the issue, walks git log/source/corpus tests to rebuild
context, and runs out of step budget before a first edit or written finding -- no permission
denials involved for 152/172, a genuine one for 149 (brief asked it to read scratch probe
files living in a *different* lane's worktree, `issue-float-build-python`, denied by the
sandbox boundary already established for issue-172/174). Read all four lanes' `.live.log`
tails directly rather than dispatching more investigation runs (evidence was conclusive).

Archived the now-redundant `stuck-issue-152-investigation` row (it would only re-derive the
diagnosis already made here). Did not redispatch any of the three -- three unrelated task
shapes stalling identically looks like a capability ceiling on DeepSeek V4 Flash for
context-heavy tasks, not three brief-wording problems, and this is the second time 149 alone
has stalled after an in-place rebrief (see the 2026-09-02 entry above). Composed one
escalation round, `docs/.grill/stalled-lanes-escalation.round.yaml`, with a per-lane
stronger-model / reshape / drop question; touched `escalated-issue-152`,
`escalated-issue-172`, `escalated-issue-149`. `trait-interface-build` (gh:174, run 3, same
rediscovery shape plus a repeat cross-worktree-denial detour into `plans/incidents/`) is one
run under the escalation threshold -- rebriefed in place instead with an explicit
incident-folder ban and a narrower first slice (just the `trait`/`impl` keywords and AST
parse, pointing at `src/parse.rs:173-176` directly) rather than escalated.

## Escalation ruling applied: all three stalled lanes redispatched on GLM 5.2 (2026-09-03)

Maintainer wizard answers on `stalled-lanes-escalation` (captured 2026-09-03 18:07, applied
same tick): all three questions -- `function-signature-matching-syntax` (gh:152),
`stdin-redesign-build-2` (gh:172), `float-format-research` (gh:149) -- ruled **Stronger
model**, none reshaped or dropped. Redispatched all three in their existing worktrees with
`OPENCODE_MODEL=openrouter/z-ai/glm-5.2` (confirmed live via `opencode models`), same task
scope as before with the prior stall summarized in-brief so the run doesn't spend its budget
re-deriving what's already known. `float-format-research`'s brief still carries the
probe-file read that's been sandbox-denied every prior run (the maintainer picked
"Stronger model," not "Reshape," for that question specifically, despite the option
description flagging that a model bump alone won't fix a permission boundary) -- told the
worker explicitly not to retry that read if denied again and to fall back to public
knowledge instead. This is the first GLM 5.2 dispatch of the rollout; worth a first data
point for the eventual model-ladder comparison once these land or stall again.
## Incident: issue-http-query-sugar-research (gh:171) dead on a self-inflicted toolchain probe (2026-09-01

Root cause (from `20260901-231026-issue-http-query-sugar-research.jsonl.tail`): a long one-run session of correct desk research (board row, gh:171 body, the sources family and `Sink` in tir.rs/ty.rs, the `dsv(delim)` parameterized-source precedent, the 7-backend list in lib.rs, draft.md's streams-and-sinks decisions) then `which go node python3 jq cc rustc` to survey which backend toolchains exist on the host -- denied by the permission classifier -- and the worker exited immediately after, zero commits, zero file writes, no ESCALATION.md, no `plans/http-query-sugar-research.md`. Exactly the gh:163 (erlang-target-research) shape one lane later: an unnecessary toolchain probe on a task that needed none, followed by give-up-on-first-denial.



Classification: brief clarity, not capability, tooling, or task shape. The worker had effectively finished the research -- three syntax candidates designed in-session,and the per-backend capability claims its survey needed are public API knowledge, not host measurements. The permission gate blocked nothing the deliverable needed;`which` was as optional here as `which erl escript erlc` was for gh:163. The task shape is the same desk-review spike gh:163 landed after its rebrief. What was missing was the brief:the gh:163 fix ("no toolchain/execution needed, docs+source read only") was applied to that lane's rebrief only, never baked into the default research-spike brief `dispatch-worker.sh` hands every fresh lane, so the next research spike replayed the identical probe-then-give-up. The board row's own phrasing ("survey what request/response building blocks already exist per backend (Go net/http, JS fetch, Python urllib, Lua, Rust reqwest, native)") invites the probe;it also carries a factual wrinkle -- the Rust backend emits self-contained files with no external crates, so `reqwest` is not available to it -- that only a probe could have made worse, not better.



Rebrief (redo, not reshape or drop:the deliverable is still needed, same as gh:163). Re-dispatch into the same lane with the standard brief plus: "This is DESK RESEARCH, docs + source read-only: no toolchain or execution is needed or allowed, so do not run `which`, version checks, or any host probe (all denied). the per-backend survey is a documented-semantics comparison against src/emit_*.rs, docs/reference/, and public API knowledge, not measurements. One correction to the board row:the Rust backend cannot use reqwest -- self-contained emitted file, no external crates -- so Rust+HTTP ends at 'no HTTP, no TLS in stdlib' without new deps. Write findings to plans/http-query-sugar-research.md and commit per AGENTS.md."

Worth carrying into the dispatch template, so this shape stops needing a per-lane rebrief:make the "no toolchain/execution needed" line part of the default brief for research rows (or add `which <tool>` probes to KNOWN DENIALS). The give-up-after-one-denial behavior itself is already logged as the 30-lane-review data point (issue-108/125/133/163);this lane adds another instance, not a new class.

## Fixed: OPENCODE_MODEL redispatch not persisted, escalation ruling applied for real (2026-09-04)

Root cause of the 2026-09-03 "stronger model" ruling silently not applying (8
commitless issue-172 runs, confirmed by lane telemetry still showing
`openrouter/deepseek/deepseek-v4-flash-0731` on run 8): `OPENCODE_MODEL` was a
one-shot env var read by `opencode-worker.sh` with no persistence in
`dispatch-worker.sh`, so any redispatch that didn't re-set the env var by hand
(continuation dispatch, event-driven re-run) fell back to the hardcoded
default. Maintainer wizard ruling on `stdin-redesign-stall-escalation`
(captured 2026-09-04 20:22, applied same tick): option 1, fix the persistence
gap and redispatch for real.

Fixed `dispatch-worker.sh`: an explicit `OPENCODE_MODEL` at dispatch time is
now written to `.opencode-model` in the lane worktree; a later dispatch with
no `OPENCODE_MODEL` set falls back to reading that file if present. Redispatched
`issue-172` with `OPENCODE_MODEL=openrouter/z-ai/glm-5.2`, confirmed via
`ps` that the live worker is running `opencode run -m openrouter/z-ai/glm-5.2`
and that `.opencode-model` now holds that value, so future redispatches of
this lane (and any lane that gets an explicit model override) stay on it
without needing the env var re-supplied every time.

## 2026-09-04: stdin-redesign-build-2 (issue-172) escalated again -- brief-shape, not model

The 2026-09-04 stronger-model ruling (GLM 5.2, persisted via `.opencode-model`) was applied
and the redispatched run used it correctly, but still landed 0 commits (run 9 total). Unlike
prior runs, this one reasoned cleanly to a real structural finding: the 2026-09-03 reshape's
commit-1 boundary ("parse.rs + tir.rs only, tree stays green") cannot compile, because
`Builtin` is matched exhaustively with no catch-all arm in `check/mod.rs` and all 8
`emit_*.rs` files -- confirmed directly against `src/emit_js.rs`. Escalation round composed:
docs/.grill/stdin-redesign-shape.round.yaml (marker: escalated-issue-172). Root cause this
time is the reshape ruling's own commit boundary, not brief clarity or model strength --
future redispatches of this lane should wait for the ruling rather than retrying.
## Investigation: `issue-float-format-research` stuck at 3 commitless runs on two permission walls (2026-09-04)

Lane stats at capture: 3 runs, 0 commits, clean tracked tree, live false, only
untracked scratch files (`scratch_float_py.py`, `scratch_float_go/main.go`) left behind. Base is
`5e2fd24` (2026-09-03, the "escalate float-build-{go,python,rust} stall" commit), 72
commits behind `origin/main`. Two frozen tails survive in
`plans/incidents/issue-float-format-research-20260904/`.

Run 1 (20:21:35) was competent and progressing: read the issue, the ADR, and
`src/float.rs` (the JS reference: Rust's `f64::to_string()`, the shortest round-trip
decimal = JS's `String(number)`), grepped the backends (`Float is JS-only in every non-JS
row`), wrote and ran a Python probe successfully (repr(float): always a `.0` suffix on
integral values, fixed notation in [1e-4, 1e16), exponent form outside that with
`e+NN`/`e-NN`, zero-padded exponent digits), wrote a Go probe, and died when
`go run ./scratch_float_go` auto-refused headless. No finding committed.

Run 2 (22:52:49) found the `float-format-research` row only in `origin/main`'s
board.yaml (its own base predates the row),traced the sandbox-widening ruling via git log,
then followed the board row's "read the float-build-python lane's probe files for a concrete
lead" instruction into the sandbox boundary: all five cross-worktree reads denied,and it
stopped, commitless, without writing anything. This is the same probe-read wall the task's
dispatch note names as the prior-stall cause,and it fired after the stronger-model redispatch
commit (8891ca5, GLM 5.2),its behavior matches what the escalation entry said the brief
carried ("told the worker explicitly not to retry that read if denied again") -- if that run
was the GLM dispatch, it reproduced the stall exactly as the option description warned (a model
bump alone won't fix a permission boundary).

Diagnosis: **tooling/permission trap**, compounded by a **brief-clarity** defect, not a
capability gap(and only weakly task shape):

- **Wall #1: `go run` is denied.** Already ruled on (float-format-research-sandbox
  escalation, `c92073a`, 2026-09-03 22:53): the ruling added `opencode.jsonc`
  allowing `go run*` for this project, because `go` had no allow rule and fell to the global
  catch-all ("ask"), auto-refused headless. But that fix landed on `main`, and this lane's base
  (`5e2fd24`) predates it -- a redispatch on the current worktree would reproduce run 1's
  death verbatim until the branch is refreshed onto main.
- **Wall #2: cross-worktree reads are denied.** The board row's own lead -- the
  issue-float-build-python lane's probe files -- lives outside the sandbox boundary, denied
  every prior attempt (run 2 hit it on all five files;issue-172/174 set the precedent).It is
  a poison instruction for a sandboxed worker: it either kills the run with denials,or, were
  it allowed, it would be reading a different lane's uncommitted scratch -- an ephemeral,
  unversioned data source, not a durable lead. The 2026-09-03 escalation already told the
  stronger-model run not to retry it, which it did anyway. It must come out of the board row,
  not be carried forward.

- Not capability: both runs read the right sources and made real progress (run 1 had the
  Python leg complete and was one tool call from Go's);the survey itself is close, not hard.
-

Proposal (rebrief/reshape of `float-format-research`):

1. **Refresh the lane base onto `main` before redispatch** -- else run 1's `go run` denial
   (now fixed on main via `opencode.jsonc`) repeats from the stale base.
 The `float-build-python`/`float-build-go`/`float-build-rust` siblings carry
   `needs: [float-format-research]` (per origin/main's board),so unblocking this lane
   unblocks three.
2. **Amend the board row**: drop the cross-worktree probe-read clause entirely(denied every
   attempt;unversioned scratch is not a lead a sandboxed worker can use). Substitute the
   already-in-worktree probes as the concrete lead: `scratch_float_py.py` (run 1's complete
   Python probe)and `scratch_float_go/main.go` (its Go probe, written but never run -- now
   runnable on a refreshed base),with documented-behavior fallback (strconv.FormatFloat /
   repr(float) docs,`cargo`/`rustc` are globally allowed) if a probe is denied again.
3. **Pre-seed the reference**: state that `src/float.rs` (`f64::to_string()`) = JS
   `String(number)` = shortest round-trip decimal,and that the deliverable is per-backend
   *spelling convention*, not the digits themselves -- the digits already agree by construction.

   Run 1 re-derived this;give the next run that finding for free instead of re-burning budget.

4. **Add a landing rule**: commit each backend's guidance as soon as that backend's probe
   leg is done (one backend per commit, mirroring the float-build-* sibling reshapes),not
   hold all three uncommitted while chasing the last leg. The board row's "a partial written
   finding beats another commitless run" is already there,but both deaths came before any
   write;the landing rule only bites once the walls are gone,or.
5. If a fresh run stalls again after steps 1-4 (the go-run wall now genuinely gone):then
   reshape the row to accept the survey from documented behavior as the deliverable -- the
   formatting conventions are documented facts and the cross-language differences are themselves
   the finding, not something needing live execution to record. The empirical-verification
   requirement is the ask worth dropping, not the survey.


## Remaining pipeline problems and scaling limits, surveyed after a night of heavy autonomous use (2026-09-06)

Context: this covers one continuous stretch spanning an OAuth outage, a full host reboot, the
sandboxed plan-decompose harness (`sandbox_dispatch.py`) landing two genuinely stuck tasks
(issue-172, float-format-research), and three more tasks dispatched and landed after that. Every
item below was hit directly or confirmed by reading the relevant script, not inferred.

### 1. The serial landing queue's lock can be silently stolen by an unrelated process

`land-lane.sh` serializes all landings through `flock -w 1800 8` on a fixed path,
`/tmp/toylang-land.lock`. Tonight, `sccache` (the Rust compiler cache daemon, unrelated to this
pipeline) ended up holding an `flock` on that exact inode -- almost certainly inode reuse in
`/tmp` after the lock file was deleted and recreated at some point in this file's lifetime, with
sccache having separately opened and flocked some other temp file that happened to land on the
same inode number. Two concurrent `land-lane.sh land <N>` invocations (mine, and the
coordinator's own for issue-152) both queued behind this phantom holder for 10+ minutes before it
happened to clear. Nothing in the pipeline can detect or break this class of lock -- the 1800s
timeout is the only recovery, and every landing attempt during that window is silently stalled
with no error, just `do_wait` in `ps`. **Scaling impact**: as landing frequency increases, the
odds of colliding with some other process's transient use of `/tmp` rise; a fixed, well-known
path in a shared, high-churn directory is not the failure mode a growing pipeline can absorb
gracefully. Fix: move the lock to a path scoped to this pipeline alone (e.g.
`~/.cache/toylang-drive/land.lock`, a directory nothing else touches) so accidental inode
collisions with unrelated host daemons become structurally impossible.

### 2. `ensure_committed()`'s tracked-path allowlist has already fallen behind once, silently, with real data loss on the line

`land-lane.sh`'s auto-commit-a-green-dirty-tree path does `git add -u` then
`git add -- src tests docs site plans` (line ~104) -- a hand-maintained list of directories,
extended once already (2026-09-02, issue-168, to add `src/` for a new file `git add -u` alone
missed). Tonight, `benchmark-fasta-build`'s new files (`benches/programs/fasta.toy`,
`benches/inputs/fasta.txt`) landed under `benches/`, which is **not** in that list. Had the
worker's own exit triggered this path unattended (it would have, on any run I did not manually
intervene in), the untracked-file cleanup two lines later --
`git -C "$d" clean -fdq` -- would have **permanently deleted the new benchmark program and its
input fixture** before they were ever committed, the exact same failure class as the 2026-09-02
incident (which was about `plans/*.md` findings), just in a directory nobody had hit yet. This is
not a hypothetical: I caught it only by manually diffing the sandbox's git status before letting
the harness's own commit step run. **Scaling impact**: every new top-level content category this
project adds (a new `benches/`, a future `fixtures/`, a future `assets/`) silently re-opens this
exact data-loss window until the next incident happens to surface it, because the fix is indexed
to specific past incidents rather than to the actual invariant ("anything a worker legitimately
creates should never be swept by `git clean`"). Fix: invert the allowlist to a denylist of
genuinely-scratch top-level paths (a short, explicit list: root-level loose files, `/tmp`-style
scratch dirs workers are told to use), or simply drop the `git clean -fdq` step and instead
report untracked files in the land log for a human/coordinator glance -- the false-positive cost
of an occasional real scratch file lingering is far cheaper than silently deleting real work.

### 3. The sandboxed dispatch mechanism is completely invisible to the stuck-lane watchdog

`stuck-watch.py`'s liveness check (`live_worker_dirs()`) only recognizes a live lane by scanning
`/proc/*/comm` for a process literally named `opencode` or `claude` whose `cwd` is inside the
lane worktree. Its secondary activity signal (`lane_state()`) globs
`~/.cache/toylang-drive/opencode/*-<lane>.jsonl` for a log mtime. `sandbox_dispatch.py`'s real
work happens inside a microsandbox VM via `msb exec` -- the host-side process is `python3`, `msb`,
and short-lived `git`/`msb exec` calls, never a process named `opencode` with the lane's cwd; its
logs live at `~/.cache/toylang-drive/sandbox-dispatch-<id>.log` (plain text, not the jsonl glob
pattern) and, for the actual opencode run output, *inside the guest's own filesystem*
(`/root/opencode-run-*.log`), which the host-side watchdog cannot see at all. Result, confirmed
directly tonight: the watchdog auto-filed `stuck-issue-benchmark-fasta-build-investigation` and
`stuck-issue-benchmark-fannkuch-redux-build-investigation` for two lanes that were, at that exact
moment, being actively and (for fasta) successfully worked via the sandbox. Cost: a wasted
worker dispatch on a bogus investigation, a spurious board row that needed manual archival, and
-- in a case where nobody happened to be watching -- the real risk that a coordinator tick reads
the false "stuck" signal as ground truth and takes a destructive action (killing/reassigning a
lane that was fine). **Scaling impact**: the sandbox harness is the mechanism that actually
unblocks tasks the plain dispatch-worker.sh path cannot (permission-wall tasks, tasks needing
`webfetch`) -- exactly the highest-value, hardest cases. As sandboxed dispatch becomes a bigger
fraction of total work (which it should, given tonight's results), the fraction of false stuck-
lane alarms grows with it, unless the watchdog is taught the sandbox's own activity signals
(check `msb list`/`msb exec <name> -- stat ...` for the sandbox's mtime, or have
`sandbox_dispatch.py` write a lane-name-matching heartbeat file the existing glob can find).

### 4. Non-numeric lane slugs never trigger a direct landing -- they depend entirely on the coordinator noticing

`opencode-worker.sh`'s `fire_next()` only calls `land-lane.sh land <N>` directly when the lane
name matches `issue-[0-9]*` (a real GitHub issue number); every other lane shape (research/
benchmark/decompose slugs like `benchmark-fasta-build`, `float-format-research` -- which is now
the *normal* naming convention for a growing share of dispatched work, not an edge case) instead
just fires a generic `drive-tick.sh` and hopes the coordinator's own duty-(b) logic ("a landable
lane the event missed") notices and lands it. Tonight this worked, but only because the
coordinator's tick was healthy and its policy explicitly names this duty; for stretches where the
coordinator's own `claude -p` calls are failing (see #6) or busy elsewhere, a finished
non-numeric lane has no direct path to landing at all -- it just sits, indistinguishable from a
lane nobody has looked at, until some tick happens to have bandwidth. **Scaling impact**: as the
non-numeric-slug share of work grows, so does the population of "finished but not yet landed"
lanes silently waiting on an indirect, best-effort mechanism, with no forcing function
comparable to the direct call numeric lanes get. Fix: extend `fire_next()`'s case pattern to
also directly land any lane whose worktree exists (drop the numeric-only restriction) --
`land-lane.sh` already no-ops safely on a lane with nothing to land.

### 5. Long-lived lane worktrees produce false-negative `just check` results the pre-landing gate trusts

Twice tonight, `just check` run *inside a lane's own long-lived worktree* failed on
`native_agrees_where_it_compiles`/`rust_agrees_where_it_compiles` (an insta snapshot of which
corpus programs compile), while the identical committed state verified clean in a fresh clone.
Root cause not fully chased (most likely a stale incremental `target/` build cache reused across
many hours and many merges in the same worktree), but the practical effect is real: land-lane.sh's
own pre-check (`if (cd "$d" && just check) ...`, see #2's code) runs this exact fragile check
*in the lane worktree*, not a fresh clone -- so a perfectly good, fully-committed piece of work can
be wrongly judged "RED, not done" and skipped, exactly as happened to `draft-core-model-migration`
tonight (its own `site/public/corpus.json` diff was real, but the red verdict was worktree
staleness, not a real regression). I only caught it by manually cloning `origin/main` fresh and
re-running `just check` there for comparison. **Scaling impact**: the more lanes stay open longer
(more concurrent work, slower human/coordinator attention), the more worktrees accumulate this
kind of staleness, and the false-negative rate on this specific pre-check rises with lane
lifetime, not with anything about the actual change being landed. Fix: either `cargo clean`
lane worktrees periodically (cheap insurance, costs a slow next build), or -- better -- change
this specific pre-check to verify against a disposable clone the way the REAL landing gate
already does a few lines later, so the pre-check and the real gate can never disagree.

### 6. The whole autonomous loop has one silent single point of failure: OAuth, with no alerting

For roughly 40 minutes tonight, every coordinator tick's `claude -p ...` call failed immediately
with `Failed to authenticate: OAuth session expired and could not be refreshed`. Nothing surfaced
this beyond a line in `event-ticks.log` that looks, at a glance, identical to a healthy tick's
own stderr noise -- there is no distinct alert, no escalation, no board row, nothing that would
catch a maintainer's eye short of reading the raw tick log closely. During that window: no
landings happened, no new work was dispatched, and (worse) the retry-cap/failure-streak logic
that watches *lane* health has no equivalent watching *coordinator* health -- a lane gets escalated
to the mailbox after N commitless runs, but the coordinator silently failing its own turn N times
in a row triggers nothing. **Scaling impact**: as the pipeline runs for longer unattended
stretches, any credential/infra failure of this shape (auth expiry, API outage, a changed API key)
produces the same silent, total stop with no signal -- exactly the "full blocker in the mailbox"
gap the maintainer identified earlier this session for lane-level stalls, but here at the
coordinator level, which is more severe since it stops *everything*, not one lane. Fix: have the
tick wrapper (`drive-tick.sh`) detect its own `claude -p` auth/API failures specifically (grep the
captured stderr for the known failure strings) and, on N consecutive failures, write directly to
a place a human will see it fast (not just append to a log) -- e.g. a dedicated
`~/.cache/toylang-drive/COORDINATOR-DOWN` sentinel plus a docs/.grill/ round, since that channel
is already the established maintainer-facing escalation path.

### 7. This session itself demonstrated the risk of multiple uncoordinated autonomous loops

At various points tonight there were: the "real" self-perpetuating coordinator (a `claude -p`
session that re-schedules itself via its own tool call), a `drive-loop.sh` I started by hand
(redundant with the above, and which did not survive detached backgrounding reliably), and one to
three `sandbox_dispatch.py` runs I drove directly -- all capable of dispatching workers and
attempting landings against the same board and the same main checkout, with only `land-lane.sh`'s
own flock actually serializing the landing half of that (dispatch has no equivalent lock; two
dispatch-worker.sh calls for the same lane are guarded by the `live worker (pid $p) owns $d`
check, which is itself a `/proc` scan racy for the exact reason #3 describes). Nothing melted
down tonight, but it took deliberate manual reconciliation (checking `git log`, `board.yaml`,
which lock a process actually held) more than once to be sure two mechanisms weren't about to
double-apply the same work. **Scaling impact**: as more independent triggers for autonomous
action exist (a human starting a manual sandbox run, a scheduled loop, a webhook-driven one), the
surface for this kind of race grows faster than the coordination primitives (one flock, one
`/proc` scan) were designed for. Worth a single source of truth for "what is currently allowed to
touch this repo's landing/dispatch surface" before the next mechanism is added, rather than after
the first real collision.

### 8. Confirmed live: the stuck-lane watchdog can recursively investigate its own investigations

`stuck-watch.py` has a dedup guard against re-filing the *same* investigation twice
(`not board_has_row(f"stuck-{lane}-investigation")`), but nothing stops it from investigating a
lane whose name already IS an investigation. Caught mid-flight tonight: the coordinator's own
tick dispatched a worker into `stuck-issue-benchmark-fannkuch-redux-build-investigation` (a
perfectly normal `kind: build` row, dispatched like any other) as part of a batch of three; that
worker never ran to completion in the time since, so 30+ minutes later (`STUCK_AFTER = 30*60`)
the exact same "not live, no commits, no escalation marker" criteria that flags any ordinary
stalled lane fired again -- this time producing a board row literally named
`stuck-issue-stuck-issue-benchmark-fannkuch-redux-build-investigation-investigation`. Nothing in
the age/liveness check distinguishes "a real task nobody has worked yet" from "a meta-task about
a task nobody has worked yet"; each additional unattended cycle would prepend another
`stuck-issue-...-investigation` layer indefinitely. Older investigation rows sitting `delegated`
for 10-38h (`float-format-research-investigation`, the four `float-build-*-investigation` rows)
apparently never hit this because their worktrees no longer exist on disk (the loop only scans
`glob(LANES/issue-*/)`) -- itself further evidence of the stale-bookkeeping pattern noted
elsewhere tonight (board rows outliving their worktrees in both directions: "delegated" rows
whose work already landed, and "delegated" rows whose worktree was silently lost). **Scaling
impact**: this is the one finding here that gets structurally *worse*, not just more frequent, as
volume grows -- an investigation lane is exactly as likely to itself go unattended as any other
lane (arguably more likely, since nothing currently prioritizes clearing them), and each miss
compounds into more board noise than the miss before it. Fix: skip lanes whose name matches
`^stuck-.*-investigation$` in the scan entirely (an investigation that stalls needs a human
glance, not another investigation of why it stalled), and separately, prioritize dispatching
existing investigation rows over fresh work so they do not accumulate a queue of their own.

### Summary: what's actually blocking further scale

Ranked by how much they'd bite as volume grows, not by how loud they were tonight: **(2) the
`ensure_committed` allowlist gap** is the most dangerous (silent data loss, already proven twice
under two different directories) and cheapest to fix; **(1) the lock-file collision** and
**(6) the silent coordinator-auth outage** are the ones that most directly cap total throughput
(everything stops, nobody notices); **(3) watchdog blindness to sandboxed lanes** and
**(4) non-numeric lanes lacking a direct land path** both get worse specifically *because* the
sandbox harness (this session's main positive result) is the right tool for an increasing share
of future work; **(8) recursive self-investigation** is the one that compounds on its own even if
nothing else changes, since it feeds on the watchdog's own unresolved output; **(5) worktree
staleness** is a slow-accumulating tax rather than a hard stop. None of these are reasons to slow
down the rollout -- they are the concrete list of what the next round of hardening should target
before volume triples.
of future work; **(5) worktree staleness** is a slow-accumulating tax rather than a hard stop.
None of these are reasons to slow down the rollout -- they are the concrete list of what the next
round of hardening should target before volume triples.

## Resolved: float-build-{go,python,lua} siblings, post float-format-research (2026-09-06)

`float-format-research` (gh:149) landed, unblocking all three parked siblings. Since the
2026-09-03 escalation, the automatic pipeline independently dispatched and landed
`float-build-go` (`8ab6eaffc`, merged via `2f7cb2d`/`e312f59`): `strconv.FormatFloat` gives
shortest-round-trip digits, printer rewrites notation to match JS's `Number::toString` layout,
same pattern as the jq and native backends. Archived the `float-build-go` row (done) and its
now-redundant `stuck-issue-float-build-go-investigation` sibling (already archived by the same
pipeline run) as done: further investigation of a stall that predates the unblocking would only
re-derive a diagnosis that no longer applies.

`float-build-python`'s worker (exited 2026-09-06 13:13) found and fixed a real bug beyond the
row's original scope: `lit` emits whole-number floats as bare integers (Rust `Display`), Python
parses them as `int`, and `tl_float`'s `repr` then spells all the digits instead of the
exponential form JS prints. Fix + regression tests landed in the worktree (`just check`: 353
green), but the "firing landing tick" logged by the worker never reached `land-lane.sh` --
never appears in `land.log`. Fired `land-lane.sh land float-build-python` directly (duty (b),
event missed) rather than waiting for a retry that evidently isn't coming.

Archived `stuck-issue-float-build-python-investigation` and `stuck-issue-float-build-lua-investigation`
as redundant for the same reason as `float-build-go`'s: both investigations targeted a stall
whose real cause (missing per-backend formatting guidance) is now fixed on main. Dispatched
`float-build-lua` fresh into its existing worktree (base refreshed from origin/main by
`dispatch-worker.sh`), pointing at `plans/float-format-research.md` as the concrete lead the
prior 6 runs never had.

## Escalated: sort_by/max_by (gh:177) dropped and re-scoped, maintainer ruling (2026-09-06)

`sort-by-max-by` hit 5 commitless runs on the same failure shape: worker tries to reconstruct
already-corrupted generated code (unclosed delimiters in `emit_go.rs`, mangled punctuation in
`runtime/toylang.c`) by byte-level in-place repair (`od`/`sed`) instead of replacing the
corrupted block wholesale from a known-good reference. Per the drive skill's failure-streak
rule, escalated into a grill round (`issue-177-salvage-stall`) rather than redispatching a 6th
time. Maintainer picked option C (drop and re-scope smaller), with a freeText addendum:
split into >=5 tasks chained by `needs`, with reevaluation checkpoints between groups.

Re-filed as 7 board rows (`sort-by-max-by-tir` through `sort-by-max-by-native`), grouped by
backend-family risk rather than 1-backend-per-row: TIR/type-check plumbing alone first, then
Go+Rust-source (typed, similar shape), then Lua+JS+Python+jq (dynamic, no C helper strings),
then native/LLVM last and standalone (the actual corruption site, gated behind checking whether
`native-backend-rust-ergonomics-research` changed the plan). Three `kind: decide` checkpoints
between groups, per the maintainer's request.

The abandoned lane's worktree (`~/.local/share/toylang-lanes/issue-177`) and branch (`issue-177`)
were left in place: `git worktree remove --force` was permission-denied by the auto-mode
classifier mid-tick,and per the drive skill's rule against working around a permission denial
through another channel, no other removal method was attempted. The branch's one clean commit
(`aa4aaad`, TIR variants only,  ​23 lines) is cited as a reference for `sort-by-max-by-tir`;the
rest of that branch (7-backend dirty diff, does not compile) should not be reused. Worktree
cleanup itself needs a maintainer call(manual `rm`/`git worktree remove`, or a permission rule
change) -- not re-attempted here.

## Resolved: the fannkuch-redux-build investigation was a false positive(2026-09-06)

`stuck-issue-benchmark-fannkuch-redux-build-investigation` (this lane) is the exact auto-file
finding #3 names: the original `issue-benchmark-fannkuch-redux-build` worker was being actively
worked through `sandbox_dispatch.py` when the alarm fired, and sandbox activity is invisible to
`stuck-watch.py`'s `/proc`-and-jsonl liveness scan. The frozen evidence in
`plans/incidents/issue-benchmark-fannkuch-redux-build-20260906/` corroborates:the worker's
last recorded action (11:19:32) was a webfetch of the CLBG fannkuch-redux reference,
permission-denied, while confirming the checksum convention its reference script
(`ref_fannkuch.py`, still untracked in the lane worktree) computes. The worktree-state capture
34 min later shows runs:1, zero commits -- a sandboxed worker mid-verification, not a stalled
lane.

The row then ate itself: three runs in (this is run 3)and still nothing committed, because
each dispatch re-derives finding #3's conclusion from the same frozen evidence. Nothing a host-side
run can add here: the verdict and rebrief the row asks for are already written up in findings
#3 (sandbox blindness; fix: teach the watchdog the sandbox's own activity signals)and #8
(the recursion; this very row is its product, archived 12:54). A fourth run produces the
same non-commit.

Rebrief: archive this row as redundant. The underlying `benchmark-fannkuch-redux-build` task
is not stalled; it is blocked on the exact tooling wall sandboxed dispatch exists to absorb -- a
permission-denied webfetch of the reference it needs to confirm the checksum. Unblock it by
dispatching through `sandbox_dispatch.py` with webfetch available(the path fasta succeeded on
earlier this session),or grant the tool to a host-side run. Do not keep dispatching host-side
investigation runs at it: until finding #3's watchdog fix lands, every `STUCK_AFTER` window
re-files the same false positive.

## Ruling: retire dispatch-worker.sh, sandbox_dispatch.py becomes the only dispatch mechanism (2026-09-06)

Grilled with the maintainer after a night where every plain-dispatch permission-wall failure
(this row included) needed a sandboxed rescue anyway. Two observations converged: (1) the
`KNOWN DENIALS` boilerplate in `dispatch-worker.sh` (`plans/prompt-efficiency-review.md`,
finding #1) is a backtracking-accumulation anti-pattern that can never fully catch up to what
the sandbox already solves structurally by removing the restriction instead of teaching around
it; (2) `sandbox_dispatch.py` is a strict superset of `dispatch-worker.sh`'s capability -- nothing
the plain path could do that the sandbox can't also do. Per the maintainer: "if the sandbox pool
works just fine, then no reason to keep both. we can add it back if we find a good reason to."

**The model, kanban for agents**: `dispatch-worker.sh` is retired. Sandbox concurrency (WIP
limit) is 3, chosen from measured `lane-history.jsonl` data across the whole session (217
snapshots): max concurrent *live* workers ever observed was 5, once; the practical ceiling was 3.
The old dispatch cap of 8 was never a real constraint -- high lane counts in the board reflected
accumulated stuck/idle backlog, not genuine parallel throughput (a fact the maintainer suspected
before the data confirmed it). Every coordinator tick: if in-progress count < 3, pull the
highest-priority unblocked board row straight into a new sandbox loop. No "try cheap first" --
there is no cheap path left to try.

**No new merge-safety gate.** Re-examined and rejected a "human reviews before landing, graduate
to full-auto after 5 clean successes" design initially proposed for this ruling: the house
philosophy already has no pre-merge review for the plain path either (`drive-tick.sh`'s own
policy: "You NEVER... read diffs pre-merge... post-land review, AFTER other duties"). What
tonight's manual sandbox rescues (issue-172, float-format-research, benchmark-fasta-build,
float-build-go -- 0/4 fully clean) actually needed was recognizing the sandbox's *own* `verify()`
false negatives (jq 1.7 vs host 1.8, root bypassing permission tests, stale `target/` caches in
long-lived worktrees), not a merge-safety review -- `land-lane.sh`'s real gate (fresh clone, full
`just test`) is unchanged and was never the thing needing a human. So each sandbox loop runs the
full cycle unsupervised from day one: extract patch, apply, merge `origin/main`, call
`land-lane.sh` directly; its existing retry/escalation logic (already built) handles a red gate
exactly as it does today for the plain path. Unresolved runs route to the mailbox via the
already-built `compose_escalation()`. Push notification fires only on mailbox escalation, not on
routine clean lands (matches "nothing changed: end quietly").

**Immediate prerequisite, not yet done**: pin the toolchain snapshot's `jq` to match the host
version before turning this on -- with no human catching false negatives anymore, an unfixed
version mismatch becomes pure wasted mailbox noise instead of a one-time discovery.

**Implementation not yet landed**: `drive-tick.sh` still dispatches via `dispatch-worker.sh` and
`opencode-worker.sh`'s numeric-lane special-casing is still live. This ruling is the design;
the cutover is a follow-up.

## Bug found and fixed: green-but-unextracted runs vanished silently (2026-09-06)

`stuck-issue-156-investigation` reached GREEN on attempt 2 (398/398 tests passing, real
verified work) but `extract_result()`'s `git format-patch {base_commit}` then failed
(rc=128, reason not yet known -- the log was discarded before this fix). `main()` only
called `compose_escalation()` when `not green`, so the green-but-no-patch case fell
through both branches: `landed` stayed `False`, `escalation` stayed `None`, and the
`finally` block removed the sandbox unconditionally. The verified work was destroyed
with no mail, no round, no trace -- compare `stuck-issue-167-investigation`, which hit
the ordinary red/retry-cap-reached path in the same tick and correctly escalated.

Found and fixed independently by two coordinator ticks at the same time (this one and
another, racing on the same file in the shared main checkout -- both landed the same
diagnosis; the other tick's commit, `4d64926`, won the race and is the one on main):

- `extract_result()` now captures `/root/format-patch.log` to
  `format-patch-failure.log` in the run's workdir when no patch comes out, so the next
  occurrence has the actual git error instead of nothing.
- `main()` now escalates on `not landed` (any run that didn't land) instead of `not
  green`, so a green-but-unextracted run always produces a `docs/.grill/` round rather
  than disappearing.
- The `finally` block now keeps the sandbox alive specifically on this anomaly (green
  but no patch) for hands-on debugging, while still tearing down normally for the
  already-understood red-gate case -- matters with the host at 97% disk.

Root cause of the `format-patch` rc=128 itself is still open -- next occurrence will
have both a kept sandbox and a captured log to diagnose from.

## Likely root cause found: stale `/repo` baked into the shared toolchain snapshot (2026-09-06)

Next occurrence, as predicted: `select-materialization-research` went GREEN (398/398)
on attempt 1, `format-patch-failure.log` captured `fatal: bad object <base_commit>`,
and the anomaly-path kept `sd-select-materialization-resear` alive for inspection.

Diagnosis from the live sandbox: `git cat-file -t <base_commit>` inside the container
fails outright (object absent from its odb), yet the same hash is a perfectly normal
commit in the host-side `clone_dir` that `base_commit` was computed from (`git
rev-parse HEAD` right after `prepare_clone()`'s checkout). `git log --oneline` inside
the container shows its own history rooted several commits *behind* `base_commit`,
ending in an old, real commit from main's history rather than the fresh checkout.

`boot_sandbox()` boots the sandbox `--from-snapshot toylang-toolchain-v2` -- a full
filesystem snapshot pinned at boot, created at some earlier point specifically to
avoid rebuilding the Rust toolchain on every dispatch (see the 12G/40G-root-disk
history above). The working theory: that snapshot was captured from a sandbox that
already had `/repo` checked out (from whatever dispatch created it), so `/repo` exists
in every fresh boot before the per-run `msb copy clone_dir name:/repo` ever runs.
`msb copy` of a directory onto an existing directory of the same name appears not to
fully replace it -- the snapshot's stale `.git` (refs, HEAD, and objects) survives, so
the container ends up running against the old baked-in checkout while `base_commit`
(and everything `extract_result()` computes from it) refers to a commit that was never
actually transferred into the container's object store. This would explain both why
`just check` still runs fine (a complete, just old, repo is present) and why
`format-patch {base_commit}` fails outright (the object genuinely isn't there).

Not yet fixed or confirmed with a controlled repro -- next step, if this recurs, is to
`msb exec` into a *freshly booted, not-yet-copied* sandbox and check whether `/repo`
already exists pre-copy. If confirmed, the fix is to make `boot_sandbox()` remove
`/repo` before the `msb copy`, or copy into a scratch path and `mv` over it.

## Fix applied without a fresh controlled repro (2026-09-06)

A drive tick's trigger claimed `select-materialization-research` (gh:176) had landed,
reasoning from "worktree is gone" alone. It never had a worktree (it ran in an `msb`
sandbox, not a git worktree) and the main log has no Land commit for it -- the
tick that produced the trigger conflated "gone" with "landed" without checking. Disk
state instead showed exactly the green-but-unextracted anomaly diagnosed above: `msb`
kept `sd-select-materialization-resear` alive, `format-patch-failure.log` showed `fatal:
bad object <base_commit>`, and an escalation round was already sitting in
`docs/.grill/select-materialization-research-sandbox-blocker.round.yaml`. The row was
left `delegated`, not archived.

Applied the fix this diagnosis already named (`boot_sandbox()` in
`.claude/scripts/sandbox_dispatch.py` now runs `rm -rf /repo` in the guest before the
`msb copy` of the real clone) without first doing the suggested fresh-boot repro --
the existing evidence (stale, behind-history `.git` surviving under a directory-copy
of the same name, exactly matching a leftover checkout baked into the toolchain
snapshot) was specific enough that the scratch-path alternative wasn't needed. Next
green-but-unextracted run (if any) will confirm or refute this from the resulting
`format-patch-failure.log`.

## Two more green-but-unextracted rows, real cause is an expired API key (2026-09-06)

A drive tick's trigger again read a gone-worktree row (`messagecard-flow-defensive-render`,
gh:173) as landed. It hadn't -- no Land commit on main, and `msb list` showed
`sd-messagecard-flow-defensive-re` still alive, kept by the anomaly path. Inspecting the
live sandbox (`msb exec ... cat /root/opencode-run-*.log`) found the actual cause: both
the plan round and the build turn failed immediately with `Error: API key expired`, so
opencode never made a single edit. `git status --porcelain` inside the sandbox was
clean and `HEAD == base_commit` exactly. `just check` then trivially passed against the
untouched repo, and the dispatch script's GREEN-but-no-diff path treated that as the
already-known "green but no patch extracted" anomaly and wrote an escalation round --
but that round's auto-generated boilerplate ("a patch exists... close to green") is
false for this case: there is no patch and never was one. Deleted the misleading round
and replaced it with `docs/.grill/opencode-api-key-expired.round.yaml`, which names the
real cause and asks the maintainer to renew `OPENROUTER_API_KEY` (or explicitly pause
sandbox dispatch until it's renewed).

Second row, `batch-type-design-research`, hit the *other*, already-fixed bug
(`format-patch: fatal: bad object`) instead -- its log has no `rm -rf /repo` step before
the `msb copy`, meaning this particular dispatch process started running with the
pre-17b0f14 `sandbox_dispatch.py` already loaded in memory before that commit landed,
so it never picked up the fix despite running afterward. Its escalation round had the
same false "close to green" framing and was replaced too. Once the key is renewed,
both rows just need a fresh redispatch -- there's no patch to resume from and no
"stronger model" that would help either one; the `sandbox_dispatch.py` fix from the
prior incident is already in place for whichever dispatch happens next.

Lesson for future ticks: `msb exec`-ing into a still-alive anomaly sandbox to read its
actual opencode/build logs is cheap and finds the real cause; trusting the
auto-generated escalation-round boilerplate at face value would have sent the
maintainer a "stronger model or hand to human" choice for a problem neither option
touches.

## Ruling: opencode invocation failures now surface as an explicit build-turn failure (2026-09-06)

`run_build_cycle()` in `sandbox_dispatch.py` never checked whether `opencode run` itself
actually attempted the task -- only whether `just check` passed afterward. An expired
`OPENROUTER_API_KEY` made every call a no-op, so `just check` trivially passed against
an untouched repo and the run reported GREEN with nothing to extract (see the incident
above). Added `fatal_api_error()`: checks each build turn's own opencode log tail for
known hard-failure substrings ("API key expired", "invalid_api_key", "Insufficient
credit", "insufficient_quota", "rate limit exceeded") and, on a match, records that
turn as a failed `Attempt` and stops retrying immediately -- retrying against the same
dead key/quota wastes the retry cap on something no amount of attempts fixes. This
routes straight into the existing escalation path instead of the misleading
green-but-no-patch anomaly. Deliberately scoped to `run_build_cycle()` only for now
(the confirmed, reproduced failure mode); `plan_phase()` has the identical gap but
already degrades gracefully to "trivial" on a missing `verdict.json`, so a dead key
there still gets caught once the build cycle runs -- lower priority, not yet fixed.

## Rescued a real fix from a pre-`rm -rf /repo`-fix sandbox instead of discarding it (2026-09-06)

`stuck-issue-erlang-toolchain-empirical-research-investigation` (gh:170) was another
green-but-no-patch anomaly, kept alive by the anomaly path. `msb exec`-ing in found the
same stale-`/repo` `format-patch: fatal: bad object` bug already fixed in `17b0f14`
(this sandbox's dispatch process had started before that fix landed, same class as
`batch-type-design-research` above) -- but unlike that row, this one's own `git log`
inside the guest showed 4 real commits ahead of a commit (`15e440e`) that does exist on
the host. Two were already-landed content (identical hashes to commits already on
`origin/main` -- board/ruling writeups), one was pure garbage from the *original*
stale-snapshot bug (a stray `repo` gitlink entry auto-committed by `ensure_committed()`'s
untracked-staging step), but one, `d5b9785`, was a genuine, small, well-formed fix:
`tests/streaming.rs` tolerating `BrokenPipe` when the native backend's child process
exits mid-write instead of asserting a clean write always succeeds. Extracted just that
commit with `git format-patch 15e440e -o /root/` inside the guest, applied it onto a
fresh `issue-stuck-issue-erlang-toolchain-empirical-research-investigation` lane with
`git am -3`, and landed it normally through `land-lane.sh land` -- pushed as `04cae72`.
The investigation's own prose write-up (what the task actually asked for in
`plans/opencode-rollout.md`) was not among the rescued commits and was lost with the
sandbox before this was noticed; the concrete fix it produced was not.

## Four pending grill escalations were harness bugs, not real decisions (2026-09-06)

A user question ("do I actually need to manually answer those grilling blocks?") prompted
re-checking every open `docs/.grill/*-sandbox-blocker.round.yaml` against root causes found
earlier tonight, rather than assuming each one needs a maintainer ruling. Found four that were
not real decisions at all:

- `stuck-issue-156-investigation` and `draft-access-model-migration`: the already-fixed
  green-but-no-patch bugs (stale `/repo`, `17b0f14`; zero-progress-not-checked, this commit's
  sibling fix below).
- `stuck-issue-167-investigation` and `benchmark-fannkuch-redux-build`: both failed on the exact
  same pre-existing flaky test, `tests/streaming.rs::assert_refuses_non_utf8` (via
  `lua_refuses_non_utf8_via_lines`/`py_refuses_non_utf8_via_lines`) -- the BrokenPipe race fixed
  in `04cae72`. Both runs predate that fix.

Deleted all four stale rounds and reset their board rows to `status: todo` so the normal
ready-row dispatch picks them up fresh once a sandbox slot is free -- no redispatch fired
immediately since WIP was already at cap 3. `float-build-lua-stall-2` and
`issue-177-worktree-cleanup` were left standing: the former's specific *framing* (persisting
`OPENCODE_MODEL` across a `dispatch-worker.sh` redispatch) is obsolete under the sandbox-only
model, but the underlying need (a stronger model for this lane specifically) still requires an
explicit `--model` override sandbox_dispatch.py's normal dispatch invocation does not carry
today; the latter is a genuine permission-denial escalation that needs the maintainer's decision
by design (never work around a blocked action through another channel).

## Bug found and fixed: a build turn that touched nothing still reported GREEN (2026-09-06)

`draft-access-model-migration` went GREEN on attempt 1 with zero retries, but the sandbox's own
`HEAD` was still exactly at `base_commit` and `git status` was completely clean: the build turn
spent its whole budget writing throwaway repro scripts under `/tmp` (outside the repo) and
reading source, never touching the actual deliverable. `just check` trivially passes against an
untouched tree, and nothing had checked whether the tree was actually touched before trusting
that pass -- the same structural gap as the already-fixed API-key case, but broader: this
happens with a perfectly valid key and a model that genuinely engaged with the task, just never
wrote to a real file. Fixed with `zero_progress_since_base()` (`HEAD` unmoved from `base_commit`
AND `git status` clean) checked before `verify()` on every build turn -- safe unconditionally
since a `kind: build` row always implies some real diff.

## float-build-lua-stall-2: the real blocker was a missing algorithm, not model strength (2026-09-06)

Asked to investigate directly rather than pick from the escalation's A/B/C rather than guess.
Traced every prior stall to the same root cause: `src/emit_lua.rs` has exactly three sites that
need a real Float implementation (`show()`, `expr()`'s `Kind::Float` arm, and a missing `Type::Float`
branch in `arith()`), and Lua has no built-in shortest-round-trip float formatter to lean on the
way Go's `strconv.FormatFloat(v, 'e', -1, 64)` does -- every run's budget went to git archaeology
(re-deriving the JS reference commit, diffing sibling lanes) because nobody had ever handed the
dispatched model a WORKING formatting technique, so it kept trying to discover one from history
instead of implementing one. Verified by hand (`lua5.4`) that the standard technique -- increase
`%e` precision one digit at a time, stop at the first that round-trips via `tonumber()` -- 
produces exactly the right shortest digits, and that Lua's native float division/comparison
already give IEEE semantics with no guard needed (same as JS, unlike Int/Int64). Wrote a brief
handing over the three exact code sites, the verified-working formatting snippet, and Go's
`tlShowFloat` as the direct port target for the ECMA-262 notation-relayout logic, explicitly
forbidding the git-archaeology pattern that burned every prior run's budget. Redispatched with
`--model openrouter/z-ai/glm-5.2` as cheap insurance (the original 2026-09-03 ruling's intent,
never actually delivered before due to the OPENCODE_MODEL persistence bug -- moot now, since
sandbox_dispatch.py takes `--model` directly per invocation). Deleted the stale
`float-build-lua-stall-2.round.yaml` -- this was a brief-quality problem, not a stronger-model-
or-drop decision.

## Two stuck-lane investigations, two different root causes, neither a real decision (2026-09-07)

Asked to investigate `stuck-issue-156-investigation`'s escalation directly rather than pick
A/B/C, then to check the other open one (`stuck-issue-167-investigation`) for the same pattern.
They turned out to be different bugs:

**156**: the row it investigated, `variant-checker-capital-first`, carried a stale `issue:
gh:156` field -- GitHub issue #156 is an unrelated matcher-naming grilling thread, confirmed via
`gh issue view`. Every dispatched run opened it, got confused reconciling the mismatch, and
burned its whole budget on archaeology. But the real answer needed no more dispatches at all:
`variant-checker-capital-first` had ALREADY LANDED -- checker enforcement in `e09f063`
(2026-09-01), the doc-corpus gate (`capital_variant_gate` in `tests/docs.rs`, matching the row's
own page list exactly) in `0f45ea0` -- and was simply never archived. Archived it directly
(status: done, dropped the stale issue: field rather than guessing a replacement), archived the
investigation with that outcome, no redispatch. `doc-migrate-capital-variants` (needs it) is the
genuine remaining work and was already correctly `status: todo` -- confirmed still needed by
checking `docs/tutorial/04-enums.md`, which still has the un-migrated `circle{r: 3}` example.
This also unblocks `variant-types-flip`, whose own title already flagged the same problem
("brief claims checker enforcement 'already done via gh:156' but variant-checker-capital-first
was still status: todo").

**167**: NOT the same bug -- `gh:167` genuinely is `module-routing-syntax-build`'s real issue
(confirmed: spun off from that exact number in the 2026-09-01 module-routing-and-erlang-target
ruling). Its dependency (`file-visibility-tracking-build`) had already landed too. It was just a
real, ready, unblocked task sitting zombied at `status: delegated` with zero work ever attempted
-- the investigating worker got lost in exploration (checked both `gh:166` and `gh:167`, git-log
archaeology) without reaching that straightforward conclusion. Reset to `status: todo` for a
normal build dispatch; archived the investigation with that outcome, no further investigation
needed.

**Systemic hardening**: `stuck-watch.py`'s `board_issue()` correctly found the row that owns a
given lane's numeric suffix -- the bug was stale DATA on that row (156's case), not the matching
logic itself. But the matching logic still cannot tell a genuine gh-number lane from a slug lane
whose numeric-looking suffix is an internal ordinal, so a future stale `issue:` field would
reproduce the exact same worker-confusion pattern. `insert_row()` now appends a one-line caveat
to the auto-generated investigation title whenever an `issue:` field is attached, telling the
worker to trust `plans/incidents/` over the link when the two disagree -- costs nothing when the
link is correct (167's case), saves a wasted run when it is not (156's case).

**Dispatch-ready and round-ready counts don't check `needs`/rulings (found live, 2026-09-07)**:
trigger listed 3 sandbox-ready build rows (`euler-slow-fragments-2`, `variant-types-flip`,
`benchmark-fannkuch-redux-build`) and "3 decide rows ready" for the round buffer. Neither count
holds up against the board: `euler-slow-fragments-2` carries an explicit maintainer ruling
(2026-09-01) to hand off to a privileged manual session, not redispatch; `variant-types-flip`'s
`needs: [matcher-totality-and-alt-design, variant-checker-capital-first]` names two board rows
that do not exist anywhere in `board.yaml` (never created), so it is not actually unblocked.
Only `benchmark-fannkuch-redux-build` (`needs: []`) was real -- dispatched that one. Checked
every `kind: decide` row's `needs` by hand: all 12 either have their `needs` already covered by
the pending `design-decisions-batch-1.round.yaml` (5 questions) or are blocked on a research row
that is still `status: todo`/`delegated`, never `done`. Composed no second round this tick --
fabricating options against unfinished research would violate the "real verified code examples"
bar. Whatever generates the trigger's "ready" counts appears to filter on `status: todo` alone,
not `needs` resolution or narrative rulings buried in the title text -- worth fixing at the
tooling level so future ticks don't have to re-derive this by hand.
## Investigation: stuck lane issue-167 (gh:167, `@(path)` module routing-arm syntax) (2026-09-06)

Lane stats at capture (frozen in plans/incidents/issue-167-20260906/): 1 run, 0 commits, clean
tree, ahead 0, live false. The lane base already carries the declared prerequisite (gh:166,
per-file origin tracking): `Origin` and the prelude merge are in the worktree, so the lane was
not blocked on it. The single run is 62 tool calls, every one read-only (31 bash, 27 read, 4
grep), zero writes, zero edits, zero commits; it ends on a denied `cat .gitignore` with no final
message.

What the single run did: read the issue and gh:166; read the whole relevant surface --
module-routing-research.md, parse.rs, ast.rs, lib.rs, main.rs, check/mod.rs, ty.rs,
check/types.rs, prelude.rs, prelude.toy, emit_toylang.rs, build.rs, draft.md, tests/corpus.rs,
tests/support/mod.rs, docs/reference/operators/match.md -- and grep'd the tree for `Origin`,
module routing, the arm-body shape, and `ty::variants`. By the end it had a complete mental
model of the change and, in the reasoning, kept circling the same un-ruled questions: which
function in the submodule is the entry point, whether dispatch is a full typed function call,
whether a submodule gets the prelude's definitions merged in, how `Origin` widens from
Program/Prelude to carry a module path, and what happens when a submodule's enums collide with
the caller's. It wrote nothing because no amount of code reading settles those.

Diagnosis: **task shape, compounded by brief clarity** -- not a capability gap, not (primarily)
a tooling/permission trap.

- The issue ruling chose the *syntax* (option C, `@(path)`) and nothing else. The semantics the
  implementation needs are the open questions module-routing-research.md leaves open: the
  entry-point convention, the strictness of the signature check, and whether a submodule ever
  has submodules. A whole-file routing primitive must say which function in the module runs, and
  no ruling says it is `handle` -- the research doc only sketches `handle` as the convention for
  candidate 1/3. The agent correctly refused to bake in answers the maintainer has not decided,
  which is the agent-invented design AGENTS.md puts at lowest authority, and so produced nothing.

- **Brief clarity** is the compounding half: the board title points at the ruled syntax but not
  at the fact that the semantics are un-ruled, so an implementer is sent into a build row that
  is really waiting on a decide step.

- **Not a capability gap**: the run's exploration was sharp and complete; it assembled the whole
  plan and stalled only on decisions that live in a decide row, not in the code.

- **Not a tooling/permission trap as the cause**: one tool was denied (`cat .gitignore`) and it
  was the last call, after the agent had already written nothing all session. Terminal detail,
  not the reason the lane stalled.

Recommendation: **reshape to a decide step, then re-dispatch the build.** The code is already
about as simple as this change can be -- `check_module` returns `ty::Enums` and `resolve_defs`
is the single module entry point, so no refactor is needed; the missing piece is rulings. The
decide step (a maintainer round) should rule: the entry-point convention for a submodule
dispatch target; whether dispatch is a full typed function call (the research doc leans strict,
to keep the backends agreeing); whether a submodule's definitions merge the way the prelude's
do, and how `Origin` widens to carry a module path; and the enum-collision policy for a merged
submodule. With those ruled, re-dispatch the build with them in the brief and an early-commit
instruction -- the standing rule this lane and its siblings keep missing -- so a partial
checker/parser diff lands before tests rather than another zero-commit run.

## New failure class: sandbox replayed another lane's already-landed commit as its own (2026-09-07)

`module-routing-syntax-build` reported `"landed": true` from `sandbox_dispatch.py`, but main has
no `Land issue-module-routing-syntax-build` commit and the lane worktree sits at `origin/main`
tip, untouched. Two distinct bugs stacked:

- **`apply_and_land()` never checked `land-lane.sh`'s exit code** -- it returned `True` as soon
  as `git am -3` applied the extracted patch onto a fresh lane, regardless of whether
  `land-lane.sh land <id>` itself actually pushed anything. Here `land-lane.sh` hit its own
  `nothing ahead of main` skip (the applied patch was already identical to `origin/main`) and
  exited 1, but the dispatch summary still claimed `landed: true`. Fixed: `apply_and_land()` now
  propagates that exit code.
- **The extracted "own work" patch was someone else's already-landed commit.** The result patch
  for this row was byte-identical to `1ac2863`, `draft-access-model-migration`'s real landed
  commit -- a wholly unrelated row that finished landing (its own `land-lane.sh` push) within the
  same few minutes, on the same shared `REPO` checkout that every dispatch clones as its `origin`.
  `boot_sandbox()`'s `rm -rf /repo` fix (17b0f14) ran correctly on both attempts, so this is not
  the earlier stale-snapshot bug. Working theory: the opencode agent, mid-task, fetched/merged
  `origin/main` inside the sandbox (plausible given the brief said a prerequisite "has already
  landed") right as `draft-access-model-migration` pushed to that same shared `REPO`, made no
  real edits of its own (`net lines so far: +0` was already logged before this), and
  `format-patch {base_commit}` then dumped the fetched-in commit as if it were the run's own
  product. Not yet confirmed with a repro (would need to catch it live);
  the fix applied now (checking `land-lane.sh`'s exit code) at least stops this class from ever
  silently reporting `landed: true` again -- the row correctly stays undispatched instead of
  being wrongly archived.

Board: reset `module-routing-syntax-build` to `status: todo` (never actually landed) and
board-archived `draft-access-model-migration` (genuinely landed as `b78fdad`, but its board row
had been stuck at `status: todo` since its *own* earlier `green-but-no-patch` reset on 2026-09-06
and was never flipped back to `delegated`/archived across its successful redispatch).

## 2026-09-07: land.lock pileup -- duplicate land-lane.sh instances queued

At tick time (12:24), `ps` showed **6 concurrent `land-lane.sh` processes** all blocked on the
same `~/.cache/toylang-drive/land.lock` flock, including duplicate queued attempts for the same
lane fired ~10 minutes apart by different ticks:

- `land draft-calls-modules-migration`: PID 3294081 (started 12:05, still running) AND PID
  3514957 (started 12:15) -- a second tick queued the identical lane before the first attempt
  had even acquired the lock.
- `land iteration-traits-scaffold-build`: PID 3294746 (12:05) AND PID 3516358 (12:16), same
  pattern.
- Also queued: `land stuck-issue-draft-records-migration-investigation` (11:50, holding the lock
  and running a `just check` via `sccache`), `land draft-matching-migration` (12:15).

Both land-failed markers this tick ("main checkout stayed busy/dirty") were almost certainly
caused by this contention -- one queued attempt finding the checkout mid-use by another queued
attempt for a different lane, not a real per-lane problem. Given 2 attempts were already queued
for each of the two flagged lanes, this tick did **not** fire a 3rd redundant `land-lane.sh` for
either -- that would only deepen the backlog against the 30-minute `flock -w 1800` timeout.
Left the existing queue to drain serially; nothing was killed.

Gap: nothing currently checks "is a land-lane.sh already in flight for this lane" before a tick
queues another one. Worth a guard (e.g. pgrep the lane name before invoking) if this recurs.

## 2026-09-07 (later same tick): root cause found -- two sibling drive-tick sessions running concurrently

This tick started with 3 land-failed markers ("main checkout stayed busy/dirty") for
`draft-calls-modules-migration`, `iteration-traits-scaffold-build`, and
`stuck-issue-draft-records-migration-investigation`. Before re-running any of them, this tick
queued `land-lane.sh land draft-calls-modules-migration` (mistake: did not check `ps` first,
unlike the previous entry above) and immediately found via `ps aux` that a duplicate was already
queued from 12:15 (PID 3514957). Worse, `ps` also showed **two live `claude -p` processes (PIDs
3940586/3940588) both started at 12:35 with the byte-identical drive-tick trigger text** --
i.e. not two ticks fired minutes apart, but two tick sessions genuinely running *at the same
wall-clock time*. The sibling was independently working the board: it queued
`land-lane.sh land draft-str-adr` (PID 3960497) during this investigation.

This tick killed its own accidental duplicate (bash PID 3954680 + flock PID 3954682) and then
stopped -- fired no further `land-lane.sh` or `sandbox_dispatch.py` calls, to avoid compounding
the pileup while a sibling session is actively mutating the same lanes/board.yaml.

Root cause is upstream of this skill: something is invoking the drive-tick loop twice
concurrently (cron/scheduler overlap, or a retry firing before the prior invocation's `timeout
2700s` wrapper exited). The land.lock guards the actual `git` mutation, so it isn't producing
corrupted state -- but it produces exactly the "busy/dirty" land-failed markers seen here, and
wastes a queue slot's worth of wall-clock per overlap. Needs a lock at the *tick* level (e.g. a
pidfile/flock around the whole drive-tick invocation, not just around land-lane.sh), not
something this skill can fix from inside one tick.

## 2026-09-07 (later still): contention cleared, resumed landing for 3 of 4 land-failed lanes

Checked before acting this time. `ps -eo pid,ppid,etimes,cmd` showed only one `land-lane.sh`
in flight (`land draft-str-adr`, PID 3960497, holding `land.lock` since 12:37, child of the
`sandbox_dispatch.py draft-str-adr` worker) and no sibling `claude -p` drive-tick process --
`/proc/<pid>/fd` on the parent `drive-tick.sh`/`claude -p` pair confirmed it was this tick's own
process tree, not a duplicate. The `iteration-traits-scaffold-build` land attempt seen moments
earlier (PID 3516358) had already exited between the two `ps` calls.

With no in-flight duplicate for any of the 4 land-failed lanes, queued 3 (house bound: up to
three landings per tick) via `nohup ... &`, each to its own log under
`~/.cache/toylang-drive/`:
- `land draft-calls-modules-migration` (also the trigger's "looks landable" lane)
- `land iteration-traits-scaffold-build`
- `land stuck-issue-draft-records-migration-investigation`

Left `land draft-matching-migration` unqueued -- bound is 3 landings, and its snapshot entry
had no ahead/dirty/live line (only the land-failed marker), so it's the least-verified of the
four; next tick should re-check and queue it if still needed.

Did not touch round composition (`inbox_records=0`, no under-filled buffer named in trigger) or
dispatch new sandbox rows (2 free slots, `draft-records-migration`/`draft-prototype-findings-migration`
already have unanswered sandbox-blocker rounds pending -- redispatching either would repeat the
prior near-miss, and this tick's bound was already spent on landing).

## 2026-09-07 (later): near-miss redispatch of draft-records-migration, caught mid-flight

Trigger's "ready" dispatch list named `draft-records-migration`, `draft-matching-migration`,
`draft-prototype-findings-migration` as safe to freshly dispatch (board status: todo, needs met).
Disk state disagreed on two counts:

- `draft-matching-migration` and `draft-prototype-findings-migration` each had real unlanded
  commits sitting in their existing lane worktree (3 and 1 commits ahead of main respectively).
  `sandbox_dispatch.py`'s `reset_lane_worktree()` force-removes the existing worktree before
  recreating it -- dispatching either fresh would have destroyed that unlanded work. Landed them
  instead via `land-lane.sh land <lane>` (queued alongside the two land-failed reruns; retry caps
  were all fresh, 0 prior attempts).
- `draft-records-migration` has an *unanswered* `docs/.grill/draft-records-migration-sandbox-blocker.round.yaml`
  from a prior attempt that got a verified patch to 408/408 green but stalled on a `just check`
  hurdle, and the round explicitly asks the maintainer to choose between stronger-model resume /
  human handoff / land-as-is. A plain redispatch would have silently ignored that in-flight
  decision and repeated the exact near-miss an earlier tick's log entry already flagged. Caught
  this after already firing `sandbox_dispatch.py draft-records-migration` and getting as far as
  booting sandbox `sd-draft-records-migration` and starting build turn 1 -- killed the process,
  `msb rm -f`'d the container, and removed the tmp clone/brief/log. No board or main state was
  touched.

Gap: the "ready" list (computed upstream of this tick's snapshot) checks board status and `needs`
but not (a) whether the lane's worktree already carries unlanded commits, or (b) whether a
sandbox-blocker round is already pending for that row. Both are cheap disk checks
(`git log main..issue-<id>`, `ls docs/.grill/<id>-sandbox-blocker.round.yaml`) that would have
prevented this without a model needing to notice. Worth adding to whatever builds the ready list.

## 2026-09-07 (later still): five land-failed markers, four already had live retries queued

Trigger repeated the same "ready to dispatch" list (`draft-records-migration`,
`draft-matching-migration`, `draft-prototype-findings-migration`) and reported
`issue-draft-calls-modules-migration` as "looks landable (worker exited)". Disk disagreed on both:

- All three "ready" rows still carry the same unlanded-worktree / unanswered-round blockers
  documented in the entry above -- nothing changed since then. No dispatch this tick.
- `ps aux` showed four `land-lane.sh` processes already running (queued on the shared
  `land.lock` flock, not stuck): `draft-matching-migration`, `draft-str-adr`,
  `draft-calls-modules-migration`, and a combined `iteration-traits-scaffold-build
  stuck-issue-draft-records-migration-investigation` run. Each of those four lanes also had a
  land-failed marker on disk, but the marker predates (or matches) an already-in-flight retry --
  redispatching any of them would have queued a second `land-lane.sh` for the same lane against
  the same flock, serving no purpose. Left all four alone.
- `draft-prototype-findings-migration` had a land-failed marker (13:25) with no live process --
  its queued retry from the entry above had already finished and hit the same transient
  "main checkout stayed busy/dirty" outcome (no LAND-FAILURE.txt, so not a real conflict/test
  failure -- this doesn't burn the retry cap per land-lane.sh's own comment). Queued one more
  detached retry for it; nothing else to do.

Gap: land-failed markers don't record whether a retry is already in flight, so the trigger/snapshot
(and a reader of the marker alone) can't distinguish "needs a redispatch" from "already queued,
just wait." Checking `ps aux | grep land-lane.sh` before redispatching is the cheap disk check
that caught it this time; worth folding into the marker itself (e.g. a pid or timestamp) so a
future tick doesn't have to re-derive it.

## 2026-09-07 (later still): queue depth pushed two waiters past the 30-minute flock timeout

Same six land-failed markers as the entry above, still unresolved. `ps -eo pid,ppid,etime,cmd`
plus `fuser land.lock` showed the queue had NOT drained since the last check -- five
`land-lane.sh land ...` invocations were queued on the shared flock at once: `draft-calls-modules-migration`
(waiting 23+ min), the combined `iteration-traits-scaffold-build stuck-issue-draft-records-migration-investigation`
run (actively holding the lock, mid-`just test`), `draft-prototype-findings-migration` (waiting
~2 min, the retry queued by the previous tick entry), plus `draft-matching-migration` and
`draft-str-adr`.

The last two had *already given up*: `land-lane-draft-matching-migration.out` and
`land-lane-draft-str-adr.out` both end with `[land] queue lock held 30+ min -- gave up (tick
will retry)`, timestamped 13:29:55 -- i.e. `flock -w 1800 8` exhausted its wait and the script
exited without ever reaching the success or failure path, so it never touched the
`land-failed-issue-*` marker (that file write only happens on a real gate/merge failure inside
the lock). The stale marker timestamps (12:45, 12:55) are from whatever attempt originally
produced them, not this timeout.

Did not requeue `draft-matching-migration` or `draft-str-adr` this tick. Re-dispatching into a
lock queue that is already 3 deep (one running `just test`, two still waiting) would almost
certainly repeat the same 30-minute give-up rather than land anything -- it's not a transient
"busy/dirty" retry candidate, it's queue depth exceeding the wait budget. Left the three
already-live attempts to drain; next tick should re-check `ps`/`fuser land.lock` and only queue
these two once the queue has room.

Gap, and a real one this time: `land-lane.sh`'s give-up-after-30-min path is silent to
everything outside its own `.out` log -- it doesn't write a marker, doesn't retry itself, and
doesn't get counted by the retry cap, so a lane can sit invisible to the board/trigger snapshot
indefinitely if every tick just re-observes the same stale `land-failed` marker without reading
the `.out` file underneath it. Two structural fixes worth considering: (a) cap how many
`land-lane.sh` invocations may be queued on `land.lock` at once (the tick already knows the
count via `fuser`/`pgrep`, so it could refuse to queue a 4th+ until depth drops), or (b) have
the give-up path itself write a distinguishable marker (`land-timeout-issue-N`, say) instead of
leaving the old `land-failed` marker looking unchanged, so a future tick's disk read doesn't
have to reconstruct this from raw `.out` files and process timestamps.

## 2026-09-07 (queue draining): one lane requeued, two left live

Re-checked `ps`/`fuser land.lock` as the last entry suggested. Queue had drained from 5 deep to
2 real contenders: the combined `iteration-traits-scaffold-build stuck-issue-draft-records-migration-investigation`
cascade (still holding the lock, now on its second lane -- `land-gate-issue-iteration-traits-scaffold-build.log`
shows its `just test` already passed 412/412 at 13:32 and a fresh `land-failed-issue-iteration-traits-scaffold-build`
marker at 13:35 shows that lane's merge hit the busy/dirty 3-minute retry ceiling, so it moved on
to the second lane, whose gate log finished 409/409 at 13:38 and was in the merge-retry window
at check time) and `draft-prototype-findings-migration` (still waiting on the flock, 11+ min in,
well inside the 30-minute budget). `fuser land.lock` also listed two extra PIDs: one was
`sccache` (inherited the lock fd from a `just test` child, not a real queue contender) and one
had already exited by the time it was checked (stale fuser cache entry) -- neither counts toward
queue depth.

`draft-matching-migration` and `draft-str-adr` confirmed via their `.out` logs to have already
hit the 30-minute give-up (unchanged since the last entry, `[land] queue lock held 30+ min --
gave up`) -- left both alone again rather than requeue into the same two live attempts; still
not worth the risk of a second cascade timeout for a queue depth of only 2.

`draft-calls-modules-migration` was different: its `.out` log (`land-draft-calls-modules-migration-retry2.log`)
shows the same 30-minute give-up, and the trigger named it explicitly as "looks landable (worker
exited)". Queued one detached `land-lane.sh land draft-calls-modules-migration` retry (pid
1191964) -- queue depth becomes 3, all bounded by the same 1800s flock wait, and the give-up
path doesn't burn the retry cap since it never reaches a real gate/conflict failure.

No fresh sandbox dispatch: `draft-records-migration`, `draft-matching-migration`, and
`draft-prototype-findings-migration` are still the same near-miss traps as the last two entries
-- each still has a live lane worktree with unlanded commits and an unanswered
`*-sandbox-blocker.round.yaml`, not a clean slot to dispatch into. Nothing on disk changed there
since the last check. No new round composed either -- 6 rounds already pending, well past the
"keep two buffered" target.

## 2026-09-08: root cause of the "18 stalled lanes" was OpenRouter credits, not disk or a key rotation

The `opencode-api-key-expired.round.yaml` escalation (fresh redispatch of the 3 unblocked
lanes -- `brief-phrasing-experiment`, `native-backend-rust-ergonomics-research`,
`trait-interface-dispatch-build` -- all failing build turn 1 in under a minute on `API key
expired`) got answered: the OpenRouter key itself was never invalid (`/auth/key` showed
`expires_at 2026-09-15`, auth succeeded); the account had run out of credit balance. Credits
topped up (`/api/v1/credits`: 160 total, ~130 used, ~30 remaining at answer time), confirmed
live against the real endpoint, not inferred from the error text. Maintainer answer: redispatch
all 18 affected lanes (the 3 above plus the other 15 left at `status: todo`) from their original
briefs, no rebrief needed -- the briefs were never the problem.

Deleted the 3 `*-sandbox-blocker.round.yaml` escalations for the already-attempted lanes as
moot: each asked "how should this blocker move forward" (stronger model / hand to human / land
as-is) on the premise of a real partial patch, but the verify tail in all three was actually
`opencode never attempted the task -- matched fatal pattern 'API key expired'` -- there was no
patch, no gap, nothing to choose between. All 18 lanes stay at `status: todo`; WIP was already
3/3 at capture time, so no immediate dispatch -- future ticks pick them up as slots free,
respecting the 3-concurrent cap rather than bursting all 18 at once.

Claude-proof? No -- an exhausted prepaid credit balance surfaces the same way regardless of
which model/vendor is behind `opencode`;the fix (top up credits) is account-level, not a
worker or brief defect. Process gap worth naming:the ORIGINAL 18-lane stall was first
misdiagnosed as disk-full (2026-09-07),and only a second, more skeptical redispatch surfaced
the real cause -- a `FATAL_API_PATTERNS`-style fast-fail on `insufficient_quota`/`402` distinct
from `key expired` would have shortened that detour.

## Investigation: stuck lane issue-draft-records-migration (2026-09-08)

Lane stats at capture (frozen in `plans/incidents/issue-draft-records-migration-20260907/`):
1 run, 0 commits, clean tree (`tracked_dirty`: 0, no untracked files, no diff), dead
~1.25h (last activity 2.6h before the capture). The incident folder holds only the state
capture -- no event-log tail survives, so what the single run actually did isn't recoverable from
that evidence alone; the diagnosis below is inference from task shape and the successful redispatch,
not a read of the run's own transcript.

The brief (`plans/brief-draft-records-migration.md`) is specific: names the three draft.md
sections ("records can be built, and a record is how several arguments travel", "record fields
keep their declared order", "record field order is not type identity"),the destination pages
(`reference/types/record.md`, with `reference/syntax/functions.md` for the unary-functions story),
the load-bearing punning-refusal requirement,and a `just check` done-gate. At dispatch time
(the original lane dispatch, 2026-09-06 23:42) draft.md was 2146 lines.

The original stall is now moot:the lane was redispatched after the account-level OpenRouter credit
outage ( the "18 stalled lanes" entry above) cleared,and that run produced `3e46c88`
("Migrate record decisions out of draft.md"), now on main:the three sections deleted from
draft.md (-162),the salvaged rationale into record.md (+15) and functions.md (+6),the still-open
"narrowing a record" thread refiled as Q41 in plans/questions.md (+10),and every cross-
reference (draft.md's parens-rule link,the research-log entry,the emuto-survey citations)
repointed. The identical brief succeeded on redispatch, which rules out a brief-clarity or
capability-gap cause for the original zero-commit run.

Diagnosis, on the four categories:**task shape**, not brief clarity, not a capability gap, not a
tooling/permission trap. The stall matches the step-budget-exhaustion-during-orientation shape
already seen on issue-170 and issue-float-build-rust:the natural approach to this task --
read the 2146-line monolith,the draft-split.md entry,the destination pages,verify coverage
against the current implementation,then write -- burns most of a one-run budget before the first
edit,and the clean tree (zero writes, no uncommitted work to salvage)is the shape of a run
that ran out of steps during orientation, not one that abandoned good work. There is no
mechanical code refactor that would shrink the per-call-site work:this is a prose-migration
(move text between markdown files, delete three sections, repoint cross-references),and the only
shared structure is draft.md itself, so there is no shared helper, flattened nesting, or removable
special case to extract. Pre-splitting draft.md into per-section files would eliminate the
landing-time line-offset conflicts that sibling draft-split rows are now causing,but that is a design
change to the draft-split protocol rather than a small reversible simplification,and it is line-
neutral, not a net reduction.

Recommendation:
- **No further action on this row**:the original stall's deliverable already landed (3e46c88,
  on main);archive `stuck-issue-draft-records-migration-investigation` as moot -- a fourth
  dispatch would only re-derive the same "already recovered" conclusion, exactly as issue-140 did.

- **Rebrief the draft-split protocol's migration sequence** for the remaining queued migration rows:
  sequence each migration as **additive-salvage-first, then subtractive-delete** -- commit the
  destination-page additions (rationale folded in, committable, conflict-free)as its own
  commit before deleting the sections from draft.md. This matches the standing commit-early
  lesson,and it also shrinks the landing-time line-offset conflicts that concurrent deletions from the
  shared monolith keep producing (the merge-conflict retries the 2026-09-08 entry above
  describes). It is a sequencing change, not a scope change -- no section list or destination
  changes, so no row's brief needs rewriting, only the order its worker is told to do things in.

## 2026-09-08 (later): three sandbox-blocker rulings applied -- two redispatched, one already moot

Maintainer wizard answers, all "A. Stronger model, same patch as the starting point", captured
2026-09-08 19:13:57 and applied same tick:

- `draft-records-migration-sandbox-blocker`: covered both the sandbox build blocker AND the
  `land-lane.sh` land-failed marker (same conflict, same evidence -- three landing attempts all
  hit `CONFLICT (content)` in `draft.md` against `origin/main`, retry cap reached). Redispatched
  via `sandbox_dispatch.py draft-records-migration --model openrouter/z-ai/glm-5.2` (fresh diff
  against current main, not a literal patch resume -- `sandbox_dispatch.py` resets every lane
  from `origin/main` per invocation, so "same patch as starting point" was applied as "same task,
  same near-miss context carried in-brief" rather than a literal `git am` of the old patch).
  Cleared `land-failed-issue-draft-records-migration` and `land-retries-issue-draft-records-migration`.
- `draft-matching-migration-sandbox-blocker`: build reached GREEN (412/412) twice but `git am -3`
  failed applying its own extracted patch onto a freshly reset lane -- genuine conflict, not a
  build problem. Same redispatch treatment, same model.
- `stuck-issue-draft-records-migration-investigation-sandbox-blocker`: **stale by the time it was
  answered** -- a later, third dispatch of this same row (not visible to the maintainer when they
  answered) already reached green and landed cleanly (`847afa9`/`27ab9c5`, archived `2f29002`).
  Cleared the round with no action; the ruling was moot.

Also archived `stuck-issue-draft-matching-migration-investigation` (board-filed after the failed
`git am -3` left that lane at `ahead=0 dirty=0 live=0`) as redundant: the sandbox-dispatch log for
`draft-matching-migration` already gives a conclusive diagnosis (green build, landing-time
conflict) -- a fresh investigation dispatch would only re-derive it, same shape as the
2026-09-02 and 2026-09-08(earlier) precedents above.

Not yet applied to either redispatch brief: this file's own "additive-salvage-first, then
subtractive-delete" sequencing recommendation (previous entry) for reducing landing-time
`draft.md` conflicts between sibling draft-split rows. Both redispatches above were already
in flight before that recommendation was cross-referenced here; whichever row picks up the next
undispatched draft-split migration should carry the sequencing instruction in its brief.

## 2026-09-09: coordinator self-inflicted container collision on module-routing-syntax-build

The trigger's "ready" list named a row (`module-routing-syntax-build`) that turned out to
already have a live `sandbox_dispatch.py` process running (started ~23:52 the previous tick,
right after that tick wrote its brief) -- the process was alive and mid build-turn-3, but
nothing in `board.yaml` (`status: todo`, no `delegated` flag) or `msb list`'s STATUS column
signaled that. The coordinator this tick did not check for a live process before dispatching a
second one on the same row id. `sandbox_dispatch.py` names its container `sd-<issue_id>` and
boots with `msb run --replace`, and `prepare_clone`'s workdir is also keyed only by issue id
(`/tmp/sandbox-dispatch-<issue_id>/repo`, unconditionally `rmtree`'d and re-cloned) -- both are
shared, unguarded resources per row id, not per-process. The duplicate dispatch was killed within
~5-8 seconds of starting, but that was enough time for its `prepare_clone` to `rmtree` and
re-clone the host-side workdir the live process depended on. The live process's next turn
(build-turn-3) then reported "ZERO file changes... HEAD never moved from the starting commit"
and escalated after exhausting its retry cap -- indistinguishable in the log from a genuine
stuck build, except that turns 0-2 had real prior activity (`opencode-run-build-0.log`,
`opencode-run-build-2.log` both non-trivial) that the escalation's own template ("converged close
to green") assumed was still live progress. Deleted the resulting
`module-routing-syntax-build-sandbox-blocker.round.yaml` as an artifact of this collision, not a
real blocker -- board row is untouched (`status: todo`), safe to redispatch clean next tick.

Lesson: before dispatching a row, check for a live process on it (`pgrep -f
"sandbox_dispatch.py <issue_id> "` or equivalent), not just `board.yaml` status or `msb list`'s
STATUS column -- neither reliably reflects "someone is already running this."

## 2026-09-09 (same tick): OPENROUTER_API_KEY failing -- both dispatches this tick went red instantly

After cleaning up the collision above, dispatched `toylang-conf-yaml-build` (never touched by the
collision) and a fresh clean redispatch of `module-routing-syntax-build`. Both reached their
retry cap and escalated within ~1-4 minutes total -- far too fast for even one real
`just check` cycle (a partial 14/398-test run alone took 6+ seconds in the pre-collision
`module-routing-syntax-build` log; a full green run takes much longer). `toylang-conf-yaml-build`'s
log makes the cause explicit and unambiguous: `FATAL opencode invocation failure on build turn 1
(matched 'API key expired') -- not retrying, this needs the key/quota fixed, not another attempt`.
The script's own fast-fail detection fired on turn 1, then (interleaved oddly in the shared log
file, but confirmed by a second full "preparing disposable clone" cycle appearing right after)
something re-ran it and it escalated again with a generic "zero file changes" verdict on the
second pass. `module-routing-syntax-build`'s clean redispatch never logged the literal "API key
expired" string but converged to the same "zero file changes, cap reached" outcome in a
comparably short time -- almost certainly the same underlying auth/quota failure, just without
the exact string match that triggers the FATAL fast-path.

Deleted both resulting `*-sandbox-blocker.round.yaml` files -- neither reflects a real per-task
design gap, both are artifacts of the sandbox pipeline being unable to call the LLM at all right
now. Left both board rows at `status: todo`, safe to redispatch once the key is fixed. Did NOT
write a design-shaped escalation round for this (nothing about it is a maintainer decision
between options) -- flagged directly to Daniel instead in the tick's chat summary.

**Action needed from Daniel: check/renew the `OPENROUTER_API_KEY` secret `msb run --secret
OPENROUTER_API_KEY@openrouter.ai` resolves at dispatch time.** Until that's fixed, every sandbox
dispatch this pipeline attempts will burn a build-turn retry cap and produce a misleading
"zero file changes" escalation instead of doing real work.

## 2026-09-09: five sandbox-blocker/decide rulings applied

The API key issue above was evidently transient or already fixed by this tick: a later redispatch
of `module-routing-syntax-build` and `toylang-conf-yaml-build` (mtimes 00:34/00:47) ran the real
toolchain successfully (full `just check` output visible in the logs, real test failures/passes,
not an instant FATAL) but still made zero file changes across all 3 build turns with
`deepseek-v4-flash-0731`, producing fresh escalation rounds at the same paths. The maintainer's
inbox answers for these two rows (captured 22:48:25 on 2026-09-08, before these fresh round files'
mtimes) were for an earlier, since-deleted instance of the same question -- per the
`draft-records-migration`/`draft-matching-migration` precedent above, applied as "same task, same
ruling intent" rather than assuming staleness, since the fresh escalation carries the identical
question shape and root cause (zero-change build turns) the maintainer already ruled on.

Applied:
- `search-and-fold-select-mechanism` round (3 questions): `search-cut-semantics` -> option B
  (`first` as an ordinary prelude fn, generic over Vec/Stream, proper `Opt`); `applicative-fold-block-syntax`
  -> option B (keep designing, next round needs real syntax options); `select-shared-mechanism-design`
  -> option 1 with the maintainer's own hedge honored literally: filed `trait-multi-impl-dispatch-build`
  to carry the real remaining scope, since the archived `trait-interface-dispatch-build` never
  actually built multi-impl dispatch despite `status: done`.
- `http-query-sugar-build-sandbox-blocker` -> option B, hand off to a privileged session.
  Prompt at `plans/issue-171-privileged-agent-prompt.md`, `escalated-issue-171` marker set.
- `draft-mutation-migration-sandbox-blocker`, `module-routing-syntax-build-sandbox-blocker`,
  `toylang-conf-yaml-build-sandbox-blocker` -> option A, redispatched with
  `--model openrouter/z-ai/glm-5.2` (fresh diff against current main, per the established
  "stronger model" convention -- `sandbox_dispatch.py` resets every lane from `origin/main`, so
  this is not a literal patch resume). `draft-mutation-migration` had no `plans/brief-*.md` on
  disk (only the sandbox workdir's `brief-sent.txt`); copied it to
  `plans/brief-draft-mutation-migration.md` for the redispatch and for the record.

Deferred: `dense-tensor-type-build-sandbox-blocker` also ruled option A, but the WIP cap (3) was
already spent on the three redispatches above (ranked by board `prio`: module-routing-syntax-build
5, draft-mutation-migration/toylang-conf-yaml-build 4 each, dense-tensor-type-build 3 lowest).
Left its round file and inbox record in place for a future tick's free slot.

## 2026-09-09 (later): dense-tensor-type-build's deferred ruling applied; two siblings escalated again

The three redispatches from the entry above (`module-routing-syntax-build`,
`draft-mutation-migration`, `toylang-conf-yaml-build`) did not all land clean:
`module-routing-syntax-build` and `draft-mutation-migration` produced fresh
`*-sandbox-blocker.round.yaml` escalations (mtimes 01:03), still unanswered in the maintainer's
inbox as of this tick -- left alone rather than redispatched a third time without a new ruling,
per the no-blind-redispatch rule. `toylang-conf-yaml-build` also still has an unanswered round
from the same batch.

Applied the deferred `dense-tensor-type-build-sandbox-blocker` ruling (option A) with a free WIP
slot this tick: redispatched via `sandbox_dispatch.py dense-tensor-type-build --model
openrouter/z-ai/glm-5.2`, same brief already on disk at `plans/brief-dense-tensor-type-build.md`
(fresh diff against current main, same "stronger model" convention -- not a literal patch resume).
Cleared the inbox record and deleted the round file.

Skipped `dsv-partials-migration` despite the tick trigger listing it as a free-slot-ready row:
its own title explicitly rules it "Blocked-open, not a live task yet" (csv/tsv move into
`prelude.toy` pending `source_in_fn` AND a partial-application mechanism actually *built*, not
just designed) -- `needs: [partial-application-system-design]` is satisfied (`status: done`), but
that row only ruled the design direction, not a build; the real blocker isn't captured in `needs`
at all. Board data-quality gap, not a dispatch decision: the `needs` graph under-represents this
row's real dependency. Left `status: todo`, did not dispatch, did not file a follow-up row (out of
this tick's scope).

**The `dense-tensor-type-build` redispatch itself came back red within ~1 minute**, and worse than
the original attempt it was meant to improve on: `plan round 1 produced no verdict.json, falling
back to trivial`, then all 3 build turns made zero file changes and no patch was extracted at all
(the first, pre-ruling attempt at least got a real 3.4MB patch close to green). This is the same
"zero file changes, suspiciously fast" shape as the `OPENROUTER_API_KEY` incident logged earlier
today, and it now spans three different rows in the same ~1-hour window
(`module-routing-syntax-build`, `draft-mutation-migration`, `dense-tensor-type-build`) -- reads
as a systemic infra problem, not three independent task-shape failures. Did not redispatch the
now-answered `toylang-conf-yaml-build-sandbox-blocker` ruling (also option A) into the same
possibly-broken pipeline; left its inbox record and round file in place rather than burning
another slot on a likely-guaranteed repeat. Flagging to Daniel directly instead of composing
another escalation round (nothing here is a maintainer decision between options).

**Action needed from Daniel: check whether `OPENROUTER_API_KEY` (or the `opencode`/glm-5.2 path)
is healthy right now** -- this is the second time today the pipeline produced fast, zero-change
"escalations" indistinguishable from real task difficulty. Until confirmed healthy, further
option-A redispatches risk wasting slots and misleading the maintainer inbox with bogus
escalations.
