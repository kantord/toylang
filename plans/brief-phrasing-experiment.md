# Brief phrasing experiment: generic reassurance in dispatch briefs

Maintainer note (composed 2026-08-31): a generic, content-free reassurance
phrasing in dispatch briefs -- variants of "it's much simpler than you think" --
seems to help planning agents find better plans. Directive: experiment on at
least 10 dispatched issues, then report progress and hold a grilling round on
the results. This file is the experiment's tracking log. The board row
`brief-phrasing-experiment` carries it on the board (tracked in-repo, no gh
issue, per maintainer ruling, 2026-09-01). The stale predecessor
`plans/prompt-experiment.md`, keyed to the retired `dispatch-worker.sh` path,
is superseded by this one.

## Methodology

- **Treatment**: exactly one generic, content-free reassurance sentence prepended
  to the brief file's task description. The sentence must add no task
  information -- if it names the work, the lane, or a fact about the task, it is
  not the treatment. The four variants below are the rotation pool; pick one
  per yes-trial, never reuse the same sentence twice in a row, so the log can
  separate "the technique" from "one lucky sentence":
  - "It's much simpler than you think."
  - "This is much simpler than it looks."
  - "It's much simpler than it sounds."
  - "Much simpler than it seems."
- **Control**: the plain brief, no reassurance sentence, everything else identical.

- **Tagging**:at dispatch time, flip a coin (or alternate) per build-kind
  dispatch. For yes, prepend a variant; for no, write the plain brief. Do
  NOT write a `reassurance: yes/no` marker line into the brief file -- the worker
  sees that text, and a "no" marker is itself a phrasing intervention on the
  control arm. The tag lives here: create the tracking row below at dispatch
  time (lane, reassurance, note), with the outcome column filled in when the
  lane resolves. Also mention the tag in the board commit message for that tick, :
  so the commit trail independently records which arm each dispatch was on.

- **Outcome vocabulary** (fill once the lane resolves):`landed clean` /
  `land-failed` / `commitless run` / `escalated`. Note any plan-quality signal
  visible from the sandbox log (e.g. the planner's verdict, plan-decompose
  rounds) in the note column, one line.

- **Stop condition**:at 10 filled rows, stop adding trials, write up the
  results against the un-reassured baseline (see the incident log in
  plans/opencode-rollout.md) and open a docs/.grill/ round asking whether to
  keep, drop, or refine the technique.



## Measurement

The per-dispatch record is the summary JSON `sandbox_dispatch.py` prints as its last
stdout line, captured to `~/.cache/toylang-drive/sandbox-dispatch-<row-id>.log` by
whoever launches the dispatch (land-lane.sh's re-dispatch redirects there, and
drive-tick.sh greps the same file for its landing check:`tail -20 | grep '"issue_id"'`).
With the instrumentation below it carries the arm tag and the plan-phase signals, so both
arms are greppable from the log in one pass. The outcome vocabulary's convergence and
over-scoping proxies are, then:
- `attempts` (build turns to green) and `landed`:the outcome column;
- `plan_rounds`:the plan-quality signals (verdict kind per round, refactor net line
  delta, rounds used), machine-readable instead of prose.



lane-telemetry.py's lanes.csv, keyed by lane name, records turns, output_tokens,
peak_context, and wall_seconds for claude sessions (coordinator ticks, and legacy enwiro
worker lanes). Whether the sandboxed opencode sessions hit it depends on the opencode
hook config, not verifiable from this checkout, so the summary JSON is the
authoritative per-dispatch record for this experiment.



## Instrumentation

`sandbox_dispatch.py` takes `--reassurance "<sentence>"`. When passed, it prepends the
sentence to the in-memory brief text the worker sees (plan prompts, build prompts, and
escalation round all inherit it via `task_text`), and records it as `reassurance` in the
summary JSON. The brief file itself is never touched -- a marker line in the brief
file would itself be a phrasing intervention on the control arm, so the arm tag lives in
the summary JSON, per the tagging rule above. Use exactly one of the four rotation-pool
sentences, and never the same one twice in a row: pass it at dispatch time, and also
record it in the board commit message for that tick, as the methodology already requires.
Control: plain dispatch, no flag, `"reassurance": null`.

The summary's `plan_rounds` list records one entry per plan-decompose round: the verdict
kind(`trivial` / `refactor-first` / `split`, or null when no verdict.json was produced), and
for refactor rounds, the verified net line delta(`refactor_net`) and whether `just check`
passed(`refactor_verify_ok`). That is the "planner's verdict, plan-decompose rounds" the
outcome vocabulary asks the note column to capture, machine-readable instead of prose.



## Trials

| lane | reassurance | outcome | note |
|------|-------------|---------|------|
| land-lane-lock-sccache-inode-reuse-fix | yes ("It's much simpler than you think.") | pending | dispatched 2026-09-07 |