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

- The session's process has exited (check for a `claude` process whose cwd is the worktree),
  or the tree is clean with every planned commit present. A live session with a clean tree
  may still be verifying -- prefer waiting for exit over racing it for the cargo lock.
- Run `just test` in the worktree. A red suite goes back to the session (or gets fixed here
  if the session is gone and the fix is small); never review a red branch as if it were done.

## 2. Review on both axes, plus style

- `/code-review` against `main`, with the spec sources named explicitly: the GitHub issue,
  the relevant `plans/*.md` steps, the draft.md DECIDED section, and any governing ADR.
- `/style-review` on the branch's changed files.
- Review subagents run on sonnet (`model: "sonnet"` on the Agent call); the coordinator's
  own judgment of their findings is where the big model earns its keep.
- Judge the findings. Hard violations and small fixes: fix them on the branch, committing
  per AGENTS.md (provenance lines, `Co-Authored-By` trailer). Non-blocking findings and
  spec gaps: file follow-up GitHub issues rather than blocking the merge. Genuine design
  questions: stop and escalate -- that is the one case the user wants to hear about early.

## 3. Merge locally, then push

- `git merge <branch> --no-ff` from the main checkout, with a merge message naming what was
  reviewed and where follow-ups went, plus the trailer. NEVER in the same shell command as a
  `cd` into the worktree: a compound "cd worktree && commit && merge" runs the merge INSIDE
  the branch, silently merging nothing (it happened twice in one day -- both times the close
  comment fired against a merge that had not reached main). Start the merge as its own
  command from the repo root, and verify `git status -sb` says main first.
- On conflicts: resolve each conflicted path explicitly and NEVER `git add -A` while a merge
  is in progress -- it once staged conflict markers into board.yaml unseen. Structured files
  (board.yaml especially) get validated AFTER resolution and BEFORE the commit, with the
  validation hard-gating the commit (`&&`, not a newline).
- Re-run `just test` on main after the merge.
- Push main after the green suite (standing authorization, 2026-08-29, "at least for now"):
  ordinary pushes only, never force, never branch deletion. Keeping origin current is what
  lets the next dispatch cut a fresh worktree without a human round-trip. Removing the
  enwiro env (`enw rm`) stays the user's -- it is safe only after the merge, and never with
  unmerged work.

## 3b. Update the board and the env

If `plans/board.yaml` exists: set the landed task's row to `status: done` (committed together
with the merge), and add rows for any follow-up issues the review filed -- `build` rows
usually, or a `decide` + `build` pair when a finding needs a design call first.

Mark the enwiro environment done as part of landing: `enw mark done --env '<name>'`. Closing
the GitHub issue (with a landing comment naming the merge commit) belongs here too, once the
merge is pushed or the user has said pushing is theirs. Removing the env (`enw rm`) stays the
user's call.

Marking done does not end sessions: check for claude processes whose cwd is under the landed
worktree (a fresh idle one can even appear later, spawned by a workspace visit) and name any
lingerers in the report so the user knows the window is safe to close. Never kill an
interactive session yourself; it is the user's window.

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
