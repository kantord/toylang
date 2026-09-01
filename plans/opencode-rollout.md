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
