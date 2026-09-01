---
type: Playbook
name: land-delegated-work
description: Review and merge a finished delegated session's branch without bothering the user. Use when a delegated agent session (enwiro-delegate flow) has finished its work, or the user asks to land/merge an agent's branch. Runs code-review and style-review, fixes findings, merges locally, and reports once.
---

# Land a delegated session's work

The user's standing instruction: when a delegated agent finishes, review and merge its work
yourself, and report back only when it is landed or when a decision genuinely needs them.
Do not ping for intermediate progress or ask permission to review or merge.

## 1. Confirm the work is actually finished

- The session's process has exited (check for a `claude` or `opencode` process whose cwd is
  the worktree -- opencode is the delegation default since 2026-08-30, claude covers
  in-flight pre-ruling lanes), or the tree is clean with every planned commit present. A
  live session with a clean tree may still be verifying -- prefer waiting for exit over
  racing it for the cargo lock. An opencode worker's quality problems found during landing
  (review findings traceable to the worker, red suites, half-done work) additionally get a
  row in plans/opencode-rollout.md's incident table.
- Run `just check` in the worktree (the full `just test` runs at landing). A red check goes back to the session (or gets fixed here
  if the session is gone and the fix is small); never review a red branch as if it were done.

## 2. Review: the coordinator reads the diff itself. No review agents. (maintainer rule, 2026-08-30)

Review subagents and panels are RETIRED -- no single-agent reviews, no two-agent
panels, no fable adjudicators, for any branch on any tier. With workers this cheap,
review agents were becoming the dominant cost of a landing, and the tiered-panel
history (a panel once cost more than the work it reviewed) ends here.

What review IS now:

- The mechanical gates: the suite, `toylang fmt`, `.claude/checks/run.sh`, clippy on
  touched code. Non-negotiable floor, and they already ran or run here.
- The landing coordinator reads the FULL diff itself against the spec sources -- the
  GitHub issue, the relevant `plans/*.md`, any governing ADR -- with effort scaled by
  judgment, not by procedure: a rename sweep gets a skim, a backend-semantics diff
  gets a careful read. The reading happens in this session, not in a spawned one.
- Judge what you find. Hard violations and small fixes: fix them on the branch,
  committing per AGENTS.md (provenance lines, `Co-Authored-By` trailer). Non-blocking
  findings and spec gaps: file follow-up GitHub issues rather than blocking the merge.
  Genuine design questions: stop and escalate -- that is the one case the user wants
  to hear about early. Worker-quality findings on opencode lanes also go in
  plans/opencode-rollout.md's incident table.
- If a landing genuinely feels beyond a solo read (rare; cross-backend semantics with
  a conflicted merge, say), the escalation is TO THE MAINTAINER in the report -- not
  to a review agent.
- Anything that boots a dev server: kill YOUR server by PID only -- never
  `pkill`/`killall` by process name. Name-based kills took down the coordinator's
  annotations server (and once the maintainer's own) four times in one day.

## 2b + 3. The serial landing queue: lane -> main (maintainer design, 2026-09-01, superseding the size-driven accumulator pipeline of 2026-08-30)

One lane at a time, straight onto main, behind the full `just test` -- the suite is
about three minutes, so batching lanes into accumulators bought nothing and cost
blame ambiguity (a red batch could not name its breaking lane). All plumbing lives
in `.claude/scripts/land-lane.sh`:

- **Landing is event-driven and deterministic.** A worker's exit fires
  `land-lane.sh land <N>` by itself (opencode-worker.sh's EXIT trap); no model sits
  in the happy path, and the merge message is generated from the lane's own commit
  subjects. To land manually, run the same command -- detached with nohup when you
  do not want to wait on the suite. Lands serialize on a flock.
- **The gate runs in a throwaway worktree**; main is touched only after green, then
  pushed, then the lane worktree and branch are removed by the script.
- **A conflict or red gate never blocks the queue.** The script writes the evidence
  to LAND-FAILURE.txt in the lane worktree, re-dispatches the lane's own worker
  with a templated repair brief (cap: 2 automatic retries), and moves on. The third
  failure leaves a `land-failed-issue-<N>` marker the tick escalates to the
  maintainer.
- **Review happens post-land** (maintainer ruling, 2026-09-01): read the newest
  Land commit's diff on main and file follow-up rows or issues for real problems --
  never block or gate the merge on a model read.

## 3b. Update the board and the env

If `plans/board.yaml` exists: run `.claude/scripts/board-archive.py <row-id>...` -- it does the
issue-#113 move (terminator-anchored cut from `plans/board.yaml`, append to
`plans/board-archive.yaml` with `status: done`, both files validated) in one deterministic
step; commit its result. Add rows for any follow-up issues the review filed -- `build` rows usually, or a `decide` +
`build` pair when a finding needs a design call first.

The worktree's fate depends on which kind it is:

- **toylang-lanes worktree** (`~/.local/share/toylang-lanes/issue-<N>`, the enwiro-free
  default since 2026-08-30): the coordinator removes it right after the merge is pushed --
  `git worktree remove <path>` from the repo root. The branch ref and commits live in the
  main repo's .git, so nothing is lost and disk stays flat. Never remove one that is
  ahead of the merge or dirty.
- **Pool lane** (gh:124, legacy; `lane:` on the row): return it to the pool:
  `enw mark ready --env 'toylang@lane-<N>'`. The pool takes no new dispatches; once its
  in-flight lanes land it is inert.
- **Per-issue enwiro env** (legacy claude flow): `enw mark done --env '<name>'`; removing
  it (`enw rm`) stays the user's call.

Closing the GitHub issue (with a landing comment naming the merge commit) belongs here
too, once the merge is pushed or the user has said pushing is theirs.

Marking done does not end sessions: check for claude processes whose cwd is under the landed
worktree (a fresh idle one can even appear later, spawned by a workspace visit) and name any
lingerers in the report so the user knows the window is safe to close. Never kill an
interactive session yourself; it is the user's window. A pool lane's idle session is the
exception in spirit only: it is the warm-reuse asset, so report it as such -- still never
kill it, and never launch a second session into its worktree while it lives (the known
two-sessions-one-worktree race): a live lane that fails the reuse rules is simply not
dispatched to until its process is gone.

**A landed branch can keep growing.** If the session is still alive at landing, its next stop
fires the hooks, which can drive lesson-writing and cleanup commits AFTER the merge (it has
happened: the first two lessons in the bundle arrived that way and sat unmerged until an
audit). Until the worker process is gone, re-check the branch for commits past the merge at
each tick, and sweep any final growth with a follow-up merge before the env is truly
finished.

## 4. Report once

One consolidated report: what landed, test counts, review findings and what happened to each
(fixed / follow-up issue / escalated), and anything genuinely needing a decision. No
play-by-play before that.
