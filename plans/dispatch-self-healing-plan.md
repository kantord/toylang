---
status: proposed
---

# Why 12 rows sat STUCK for two days, and what has to change so it cannot recur

Written 2026-09-14 from the raw dispatch artifacts (`plans/dispatch-log.csv`, every
`~/.cache/toylang-simple-dispatch/results/*-full-agent.log` and `-self-report.txt`, the
tick logs under `~/.cache/toylang-drive/`), after the maintainer asked why the board showed
12 rows in progress while the drive loop reported nothing to do on every tick.

The first half is the incident. The second half is the plan: fix the defect, then close
the specific observability gaps that let a harness bug masquerade as twelve unrelated
task-scope problems through roughly 460 commits of pipeline work.

## What actually happened

Nothing was running. All 12 `delegated` rows had a terminal STUCK in the dispatch log,
had been folded into the pending `stuck-row-triage-2` round as escalation questions, and
every tick since then correctly found "already escalated, nothing to do." The reboots
killed nothing; every run had ended on its own hours before the first one.

The STUCK verdicts themselves were wrong. Two rules in `agent_loop.py` produced almost
all of them:

**The no-progress cutoff ends an attempt after 12 consecutive turns with no repo change**
(`--max-turns-without-progress`, default 12; in practice it fires on turn 11). The worker
makes one or two tool calls per turn, so a task that needs more than about 15 file reads
before its first write is ended mid-exploration every time. What follows is mechanical:

1. Attempt 2 discards attempt 1's transcript by design (a cost optimisation recorded in
   the design doc's third review cycle), re-reads the same files from zero, and is cut off
   at turn 11 again.
2. Both attempts print the identical tail `(no changes, no verify run)`. The dedup rule
   ("verify output matches a previous attempt, not retrying further") fires, and the run
   ends STUCK after 2 of its 3 attempts.
3. The self-report question is asked and the model answers honestly: "I was still in the
   exploration phase and hadn't made any edits." The tick's trigger text presents that
   with exactly two options, "narrower redispatch or decide-row escalation," and the
   coordinator picks one.

**The dedup rule also ends runs that made real progress.** The sandbox snapshot has no
`tsc`, so `just check` fails on `tsc_accepts_the_declaration_and_consumer` on every
attempt with the same tail. Two attempts that each made real, verified edits therefore
look identical to the dedup rule. `tensor-transpose-build` run `36d59b9d` finished five of
seven backends over 20 minutes and three attempts, and was classified STUCK by this rule.
Its patch is still on disk, unlanded.

How the 54 logged runs actually ended (counted from the `-full-agent.log` files):

| ending | runs |
|---|---|
| dedup STUCK, every attempt cut off at turn 11 with no edits | 35 |
| dedup STUCK, one attempt made real edits, the next two were cut off at turn 11 | 9 |
| dedup STUCK, every attempt made real edits and hit the identical `tsc` RED | 1 |
| VERIFY_FAILED after all three attempts | 1 |
| GREEN (4 real rows, 4 smoke or validation runs) | 8 |

46 of the 54 runs hit the no-progress cutoff at least once. In the nine mixed runs, a
first attempt that edited files and failed only on `tsc` was followed by two
exploration attempts that started from a discarded transcript, so the run's real work
was classified away by the two empty attempts after it.

Two of the four real GREEN runs also hit the cutoff in attempt 1 and only succeeded because
attempt 2 happened to explore faster. The pipeline can currently complete a task only if
the model reaches its first edit inside about eleven turns.

## Why it was not self-healing, and why nobody found it

This is the part worth reading slowly, because the fix for the bug is twenty lines and
the fix for the blindness is not.

**The give-up reason was never recorded as data.** `dispatch-log.csv` has `status`,
`cost_usd`, `patch_path`. It does not say who decided to stop: the model (said DONE), the
harness (no-progress cutoff, dedup, max turns), the environment (tsc), or the clock. The
turn-11 signature is visible in every `-full-agent.log`, but only to someone who opens
forty log files and notices the same line number. No tick, lint, or script ever
aggregated it.

**Every individual failure was cheap, and cheap looked like success.** The design doc's
cost work was aimed at expensive failures ("$0.174 for one task"). A $0.012 STUCK with
zero edits was reported as "direct evidence the cost fixes work as designed." Forty of
them in a row never tripped anything, because nothing measured the rate of
zero-edit runs. A run that spends nothing and changes nothing is not a cheap failure; it
is a pipeline that did not run.

**The self-report told the truth and the reader had no vocabulary for it.** On
2026-09-11 the first self-report ever captured said, verbatim, "that's the blocker, not
the task's scope." The trigger text that carries self-reports to the coordinator offers
two verbs: redispatch narrower, or escalate. Neither is "the harness stopped me." So a
correct diagnosis of a harness defect was routed, every time, as a scope decision, and
scope decisions go to the maintainer as round questions. The escalation machinery worked
perfectly and amplified the wrong signal.

**The pattern was noticed once, filed as an open question, and nothing watched it.** The
design doc, same date, on `toylang-conf-yaml-build`: "cut off by the no-progress cutoff
at turn 12 each time ... close to (but not at) the real edit site ... or the no-progress
cutoff firing slightly too early ... Left as an open question rather than guessed at,
worth watching if a similar shape recurs." It recurred in 46 of the next 54 runs. There is no
place a tick reads "things worth watching" from, so a correct hunch written into a
2100-line prose file was as good as never written. The approved coordinator-memory design
is the right home for this, and its build row (`coordinator-memory-pool-build`) is one
of the twelve STUCK rows, killed by the same cutoff it would have helped catch.

**Each STUCK row got a row-shaped response.** Twelve rows, three tasks' worth of
identical failure, produced seven separate round questions about seven separate task
scopes. Nothing looks at STUCK rows as a population. The one signal that was a population
signal, "3 more STUCK rows on the same task," was handled by folding them into the
existing question rather than asking why a whole task family fails at the same turn.

**The board lies by omission.** `status: delegated` with a terminal STUCK in the log
renders as "in progress" in the dev site. The tick knows the difference (its state
snapshot says `[row: STUCK $0.01]`); the human-facing view does not, so the maintainer
saw twelve healthy workers where there were zero.

**Verification differed between sandbox and host, silently.** `just check` cannot go
GREEN inside the snapshot without the worker hand-installing TypeScript from the npm
registry, which four separate self-reports describe doing. No preflight ever checked
that a clean clone passes `just check` in the sandbox before dispatching work into it.

## Plan, part 1: fix the defect (one session, about an hour)

All in `.claude/scripts/`, each its own commit, each verifiable against the existing
logs before any live dispatch.

1. **Raise the no-progress cutoff and stop discarding the transcript on it.** Default
   `--max-turns-without-progress` from 12 to 30 (equal to `--max-turns`, i.e. disabled
   until the data in part 2 says what the right number is). On a no-progress exit, keep
   the attempt's messages; the cost argument for discarding them was made when the
   cutoff was assumed to be catching genuinely unproductive runs, which the logs show it
   was not.
2. **Never dedup an attempt that made edits.** The dedup rule compares verify tails; it
   must require `moved == False` as well. Two attempts that both changed the tree and
   both failed the same environmental test are progress, not repetition.
3. **Make the sandbox baseline GREEN.** Build `toylang-toolchain-v3` with TypeScript
   present so `tsc_accepts_the_declaration_and_consumer` passes on a clean clone. Add a
   `simple_dispatch.py --preflight` that clones `origin/main` into a fresh sandbox, runs
   `just check` once, and records the result; dispatch refuses with
   `SETUP_FAILED: baseline RED` if the snapshot cannot pass its own gate. Run it once per
   snapshot change, not per dispatch.
4. **Land what already exists.** Resume `tensor-transpose-build` from its persisted
   messages (`--resume-from`, built and never once used by a tick according to the drive logs) with a
   brief that names the two remaining backends. Redispatch `sort-by-max-by-rust-wiring`
   from its 2502-byte patch through `land-patch`, since the full gate runs there anyway.
5. **Retire `stuck-row-triage-2.round.yaml`.** All seven questions rest on the premise
   that these were scope failures. Replace with one note in the round file saying so and
   pointing here, so the maintainer does not spend an evening answering them. Reset the
   other ten rows to `todo`; they redispatch under the fixed harness on the next tick.

## Plan, part 2: close the gaps

Ordered by how much of the incident each one would have caught on its own. The first
three are small and would each have surfaced this within a day.

### Record who stopped the run

Add three columns to `dispatch-log.csv` and the per-run `agent-status.txt`:

- `ended_by`: one of `model_done`, `no_progress_cutoff`, `max_turns`, `dedup`,
  `wall_clock`, `api_error`, `setup`.
- `edits`: count of turns in which the repo signature changed.
- `turns`: total turns across attempts.

`agent_loop.py` already knows every one of these at the moment it prints the human
line; this is writing the same fact to the structured surface. Every downstream
consumer (`dispatch_state.py --status`, the tick trigger, the dev site) shows them.
A STUCK with `ended_by=no_progress_cutoff edits=0` reads as what it is.

### One bundle per run, one command to read it, and evidence that outlives the cache

Today a run leaves six flat files in `~/.cache/toylang-simple-dispatch/results/`,
distinguished only by suffix (`.log`, `-full-agent.log`, `-messages.json`, `.patch`,
`-self-report.txt`), with no index, no retention rule, no link from the board or the dev
site, and nothing in git. Reading a broken task means knowing that layout, and nobody
who did not write `simple_dispatch.py` does. This incident was diagnosed with `grep`
across forty of them; that is the process to replace.

**Bundle.** Every run writes one directory, `results/<row>/<run_id>/`, holding what it
already produces plus what part 2 adds: `brief.txt` (the exact task text sent), `status.json`
(`ended_by`, `edits`, `turns`, attempts with each attempt's own ending and verify tail,
cost, base commit, snapshot name, model), `agent.log`, `messages.json`, `self-report.json`
(the structured answer) and `self-report.txt`, and `patch` when there is one. The
`dispatch-log.csv` row gains `bundle_path`. Nothing here is new information; it is the
same files given one address.

**One command.** `dispatch_state.py --show ROW [RUN_ID]` prints `status.json`, each
attempt's ending line, the self-report, and the last thirty lines of the agent log. It is
the standard way to look at a broken task, for a tick and for a human, so nobody needs the
layout. The tick's trigger text for a non-GREEN row names the command instead of pasting
the self-report alone, and the drive skill's policy text says to run it before deciding.

**Visible from the board.** A small vite plugin in `site/vite-plugins/`, following
`annotations-inbox.ts`, serves `/__dispatch/run/<row>/<run_id>` from the bundle
directory. The dev site's task card links its latest run; the run page shows
`status.json` and the self-report inline and the agent log below. This is the piece that
would have shown the maintainer twelve identical "no_progress_cutoff, 0 edits" cards
instead of twelve "in progress" ones.

**Retention.** Bundles stay out of git (a `messages.json` is 100 to 200 KB). Keep the
last three per row and every bundle referenced by a row that is still `delegated` or is
named in a pending round; `dispatch_state.py --gc` removes the rest alongside orphaned
sandboxes. When a tick escalates a row or the health line fires, it copies that run's
`status.json`, `self-report.json` and the last hundred lines of `agent.log` into
`plans/incidents/<row>-<date>/`, the folder that already exists for this purpose, so the
evidence for a decision survives cache cleanup and is readable by the next agent through
git alone. The full log stays in the cache; the incident folder holds enough to see the
shape.

### Give the tick a population view, and a third verb

`dispatch_state.py --health` over the last 20 runs: the distribution of `ended_by`, the
zero-edit rate, and the GREEN rate. `drive_tick.py` joins a health line to the trigger
whenever the zero-edit rate exceeds half or three consecutive runs share a harness
`ended_by`, and the line says what to do: "this is a harness defect; do not escalate
individual rows; open or update a harness row and hold dispatch." The two-option text in
`_process_delegated_row` gains that third option explicitly, and the drive skill's
policy text names it.

The threshold is agent-invented and should be treated as a first guess. What matters is
that some threshold exists and is checked by the machine every tick.

### Make the self-report structured as well as prose

The self-report call already runs with `tool_choice: "none"`. Add a `response_format`
JSON schema with `blocker_kind` in `{harness_cutoff, environment, scope, unclear}` and
`edits_made: bool`, keep the prose alongside. The prose is for the human; the enum feeds
`--health`. On 2026-09-11 the model would have answered `harness_cutoff` and the count
would have been visible by the third run.

### Show the dev site what the tick already knows

`TaskCard.tsx` renders `delegated` as "in progress." It should render the latest
dispatch-log row for that id: "in progress" only when `dispatch_state.py --live` lists
it; otherwise `STUCK x3 (no_progress_cutoff)`, `GREEN, unlanded`, and so on. The tick's
state snapshot already carries this per row; expose the same data to the page. A
`board-lint.py` check flags any `delegated` row with no live process and a terminal log
row older than six hours.

### Put "worth watching" somewhere a tick reads

The approved `plans/coordinator-memory-design.md` defines `footprint-conflict` and
`fact-check` facts. Add a third kind, `watch`: a one-line condition and the observation
that prompted it, e.g. "if a run ends at the no-progress cutoff within two turns of a
`grep` for the edit site, the cutoff is too low." The tick's state snapshot lists open
watches; `--health` output is checked against them. Land the pool build first (it is one
of the STUCK rows), then seed it with this incident's one watch. The design doc keeps its
role as history; watches are the part of history that has to be re-read every tick.

### Treat a cheap zero as an alarm, not a saving

The cost reporting in the design doc and in `SUMMARY` lines currently frames a low-cost
STUCK as a win. Add to the tick health line: a run with `edits=0` is counted as a
zero regardless of cost, and the summary reports `zero-edit runs` next to cost. This is a
framing change more than a code change, and it belongs in the policy text so future
review cycles do not repeat the reasoning that hid this.

### A population rule for STUCK escalation

Policy, in the drive skill: when more than two rows go STUCK in one tick, or more than
four STUCK rows are pending escalation at once, the tick does not add round questions.
It writes one harness `decide` row naming the common `ended_by`, holds dispatch, and
surfaces that single row. `board-lint.py` enforces the count: a round file with more
than four `escalation`-flow questions is a lint error with this rule as the message.

## What this is not

Not another review cycle on `agent_loop.py`. Nine adversarial rounds found real bugs in
the no-progress machinery and none of them asked whether the machinery's premise (that
12 read-only turns means the run is lost) matched the runs. The reviews had the code;
they did not have the aggregate. Part 2 is about producing the aggregate, so that the
next wrong premise is contradicted by a number on the next tick instead of by a human
with `grep` two days later.

Not a model change. Four GREEN runs and the 20-minute tensor-transpose run show the
worker can do this work when the harness lets it finish. Whether a stronger model would
explore faster is a real question, but it is the second question; the first is whether
the harness can tell the difference between a model that is lost and one that is reading.

## Provenance

Human-authored: the goal (self-healing, and closing the observability gap rather than
patching the bug), the framing that an enormous amount of prior work failed to catch
this. Derived: the incident numbers and mechanism, from the logs cited at the top; the
`watch` kind extends the approved coordinator-memory design; the three-column log
extension follows `dispatch-log.csv`'s existing shape. Agent-invented: the health
thresholds (half zero-edit, three consecutive, four pending escalations), the `ended_by`
vocabulary, the decision to disable the cutoff outright rather than pick a new number.
