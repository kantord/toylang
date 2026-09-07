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
  lane resolves. Also mention the tag in the board commit message for that tick,:
  so the commit trail independently records which arm each dispatch was on.

- **Outcome vocabulary** (fill once the lane resolves):`landed clean` /
  `land-failed` / `commitless run` / `escalated`. Note any plan-quality signal
  visible from the sandbox log (e.g. the planner's verdict, plan-decompose
  rounds) in the note column, one line.

- **Stop condition**:at 10 filled rows, stop adding trials, write up the
  results against the un-reassured baseline (see the incident log in
  plans/opencode-rollout.md)and open a docs/.grill/ round asking whether to
  keep, drop, or refine the technique.



## Trials

| lane | reassurance | outcome | note |
|------|-------------|---------|------|
| land-lane-lock-sccache-inode-reuse-fix | yes ("It's much simpler than you think.") | pending | dispatched 2026-09-07 |