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
- Run `just check` in the worktree (the full `just test` runs once per accumulator promotion). A red check goes back to the session (or gets fixed here
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

## 2b + 3. The size-driven pipeline: lane -> to-merge-* -> main (maintainer design, 2026-08-30, superseding the same-day single-wip flow)

Merging never blocks work, and the SIZE of a branch tells you its role: small
accumulators take lanes in, a full or stale one goes to main. All plumbing (verify
clean/ahead/no live worker, merge --no-ff, gate, push, worktree removal) lives in
`.claude/scripts/land-lane.sh` so the coordinator spends its turns on judgment:

- **Fold finished lanes** (after the diff read passes):
  `.claude/scripts/land-lane.sh fold <merge-msg-file> <issue>...` -- gate is `just
  check`. Several ready lanes go in ONE call; lane worktrees are removed by the
  script. The first lane seeds a `to-merge-<epoch>` accumulator (its own commits,
  aliased); later lanes fold into the LARGEST accumulator not mid-promotion.
- **Promotion is automatic and DETACHED.** A fold that crosses 600 changed lines
  (insertions + deletions vs main; typical lanes 50-700, measured 2026-08-30) fires
  `land-lane.sh promote <branch>` itself, in the background. The full `just test`
  runs in a throwaway worktree; main is touched only after green, then pushed. The
  coordinator never waits on it.
- **A stale accumulator promotes as-is**: untouched 30+ minutes (the tick trigger
  names it) means nothing more is coming -- run the promote yourself, detached with
  nohup. Under light load this IS the common promotion.
- **A red promotion leaves main untouched** and drops a `promote-failed-<branch>`
  marker the tick gate reports. Route a research/continuation dispatch (nextest
  names the breaking test, which usually names the lane); the accumulator keeps
  taking folds only after the repair -- never promote around a red suite.
- Workers cut their branches from the largest live accumulator (dispatch-worker.sh
  does this), so landed-but-unpromoted work is buildable-upon.
- On merge conflicts inside an accumulator: resolve each conflicted path explicitly,
  NEVER `git add -A` mid-merge (it once staged conflict markers into board.yaml
  unseen); structured files get validated after resolution, hard-gating the commit.

## 3b. Update the board and the env

If `plans/board.yaml` exists: run `.claude/scripts/board-archive.py <row-id>...` -- it does the
issue-#113 move (terminator-anchored cut from `plans/board.yaml`, append to
`plans/board-archive.yaml` with `status: done`, both files validated) in one deterministic
step; commit its result together with the wip merge. Add rows for any follow-up issues the review filed -- `build` rows usually, or a `decide` +
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
