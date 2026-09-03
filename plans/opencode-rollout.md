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

  before it ran out of steps. The bottleneck is the decision tree, not tenacity..
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
