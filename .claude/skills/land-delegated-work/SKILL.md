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
- Run `just test` in the worktree. A red suite goes back to the session (or gets fixed here
  if the session is gone and the fix is small); never review a red branch as if it were done.

## 2. Review, sized to the branch (maintainer rule, 2026-08-30)

The review effort matches the branch, not a fixed panel -- a two-agent panel on a
two-file mechanical diff once cost more than the work it reviewed:

- **haiku-tier rows and mechanical diffs** (renames, sweeps, config/UI tweaks, roughly
  under ~150 changed lines of non-generated code): NO review agents. The landing
  coordinator reads the diff itself against the issue; the suite, `toylang fmt`, and
  `.claude/checks/run.sh` are the mechanical gates and they already ran.
- **regular sonnet-tier branches**: ONE review agent (sonnet), briefed on spec
  correctness against the named sources -- the GitHub issue, the relevant `plans/*.md`
  steps, the draft.md DECIDED section, any governing ADR -- with style folded into the
  same brief as a secondary axis. Style is mostly machine-enforced now (fmt canonical
  form, the checks script); a dedicated style agent rarely finds blockers.
- **fable-tier, cross-cutting, or semantics-touching branches** (checker, backends,
  prelude contracts): the full two-agent panel, spec + style, as before.
- Review subagents run on sonnet (`model: "sonnet"` on the Agent call). The landing
  coordinator itself runs sonnet (maintainer rule, 2026-08-30) -- landing is mostly
  plumbing. ONLY on panel-tier branches, judgment escalates: spawn ONE fable subagent
  (`model: "fable"`) handed the panel's findings, the diff, and any semantic merge
  conflict, to adjudicate what blocks, what gets fixed on the branch, and how conflicts
  resolve; the sonnet coordinator executes its verdicts.
- Review briefs that boot a dev server MUST say: kill YOUR server by PID only -- never
  `pkill`/`killall` by process name. Name-based kills took down the coordinator's
  annotations server (and once the maintainer's own) four times in one day, eating a
  submitted grilling round each time the timing was wrong.
- Judge the findings. Hard violations and small fixes: fix them on the branch, committing
  per AGENTS.md (provenance lines, `Co-Authored-By` trailer). Non-blocking findings and
  spec gaps: file follow-up GitHub issues rather than blocking the merge. Genuine design
  questions: stop and escalate -- that is the one case the user wants to hear about early.

## 2b. Batch landings cascade (maintainer flow, 2026-08-29)

When THREE OR MORE reviewed-and-green branches are ready at once, do not merge them into
main one by one. Build a tournament instead: pair the branches, merge each pair into an
integration branch (`integration/<a>+<b>`, cut from main) in its own temp worktree -- the
pairs are independent, so their merges, conflict resolutions, and suite runs all happen in
parallel; then pair the winners, and only the final champion merges into main, taking ONE
main suite and ONE push for the whole batch. Conflicts get resolved at the pair level,
where they are smallest and main is never mid-merge; a suite failure at any node localizes
to its subtree. Board flips and issue closes all happen with the final merge. Per-branch
review stays a hard prerequisite -- the cascade accelerates merging, never judgment. With
fewer than three ready, the serial flow below is simpler and just as fast.

## 3. Merge locally, then push

- `git merge <branch> --no-ff` from the main checkout, with a merge message naming what was
  reviewed and where follow-ups went, plus the trailer. NEVER in the same shell command as a
  `cd` into the worktree: a compound "cd worktree && commit && merge" runs the merge INSIDE
  the branch, silently merging nothing (it happened twice in one day -- both times the close
  comment fired against a merge that had not reached main). Start the merge as its own
  command from the repo root, and verify `git status -sb` says main first.
- On conflicts: resolve each conflicted path explicitly and NEVER `git add -A` while a merge
  is in progress -- it once staged conflict markers into board.yaml unseen. Structured files
  (board.yaml and board-archive.yaml especially) get validated AFTER resolution and BEFORE the
  commit, with the validation hard-gating the commit (`&&`, not a newline).
- Re-run `just test` on main after the merge.
- Push main after the green suite (standing authorization, 2026-08-29, "at least for now"):
  ordinary pushes only, never force, never branch deletion. Keeping origin current is what
  lets the next dispatch cut a fresh worktree without a human round-trip. Removing the
  enwiro env (`enw rm`) stays the user's -- it is safe only after the merge, and never with
  unmerged work.

## 3b. Update the board and the env

If `plans/board.yaml` exists: MOVE the landed task's row out of `plans/board.yaml` into
`plans/board-archive.yaml`, appended with `status: done` (committed together with the merge) --
landing no longer flips status in place (issue #113). Match the row by its id terminator
(`'- id: <slug>\n'`, never a bare prefix -- see the drive skill's stall-diagnosis section) and
add rows for any follow-up issues the review filed -- `build` rows usually, or a `decide` +
`build` pair when a finding needs a design call first.

The env's fate depends on which kind it is (`lane:` on the row says pool):

- **Pool lane** (gh:124): the env is not done, its task is. Return it to the pool
  instead: `enw mark ready --env 'toylang@lane-<N>'` and
  `enw goal set --env 'toylang@lane-<N>' 'idle (pool); last: gh:<N>'`. Run
  `.claude/scripts/lane-context.py` on the worktree and record context/eligibility in
  the report, so the next dispatch knows whether the lane is reusable or due a recycle
  (the reuse rules live in the enwiro-delegate skill, section 0). Never `enw rm` a lane.
- **Per-issue env** (classic flow): `enw mark done --env '<name>'`; removing it
  (`enw rm`) stays the user's call.

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
