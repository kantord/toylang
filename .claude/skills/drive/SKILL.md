---
type: Playbook
name: drive
description: Drive development autonomously from plans/board.yaml - the ordered task board with dependencies. Use when the user says "drive", "keep going", "work the board", asks what's next, or wants autonomous development to continue while they only do grilling and goal-setting.
---

# Drive the board

## Session bootstrap (a fresh session starts here)

Cron jobs die with their session, so a new driving session re-arms them first:

1. **Reconstruct in-flight reality before dispatching anything**: for every `delegated` board
   row, check its worktree (commits vs main, dirty files, live worker via pgrep cwd). Adopt
   healthy lanes, intervene on dead ones (see the stall guidance below), land finished ones.
2. **Arm the drive tick**: a recurring cron at off-minutes (9,39 * * * *) whose prompt names
   the current lanes and both annotation stores -- poll
   `docs/.annotations/inbox.json` AND `docs/.annotations/notes.json` (compose messages and
   span notes), applying answers when 5+ minutes quiet or marked read.
3. **Arm the audit cron**: every ~5 hours (23 */5 * * *), running "The periodic audit" below.
4. Verify push distance before any dispatch (worktrees branch from origin).
5. **Start the lane watcher** (`bash .claude/scripts/lane-watch.sh` as a background task):
   it exits the moment any delegated lane is committed, clean, and eight minutes quiet, so
   finished workers become push notifications instead of next-tick discoveries. Restart it
   after every landing (it exits per event) and after every dispatch if it reported "no
   delegated lanes remain".

## Stall diagnosis, learned the hard way

The dead-worker signature: the newest file in the session's tool-results dir is its own
session-start hook message -- the worker died (usually machine suspend) and a fresh idle
session auto-spawned. A worktree whose tree is fully STAGED by a dead worker may be verified
(suite + build) and committed by the coordinator directly, with the commit message saying so;
uncommitted half-done work gets a continuation dispatch into the same env whose brief says to
read ALL issue comments and assess the existing diff. File-write mtimes and commit times are
the truth; transcript timestamps lie.

Board-editing scripts match a row id ONLY with its terminator -- `'- id: <slug>\n'`, never a
bare prefix: `- id: nullary-functions` also matches `nullary-functions-decision`, and the
falls-through `index('status: todo')` then flips whatever row comes next (it silently marked
a decide row delegated once; the audit caught it, not the edit).

Two bookkeeping rules the audits keep re-finding: a follow-up issue filed during a landing
gets its board row IN THE SAME COMMIT (an issue without a row is invisible to this loop --
four accumulated once); and an inbox record dismissed as stale gets that dismissal NAMED in
the tick's report (a silent no-op clear is the one path where maintainer input can vanish
without a trace).

`plans/board.yaml` is the single source of truth: an ordered list where position is priority.
Each entry: `id`, `title`, `kind: build | decide`, `needs: [ids]`, `status: todo | delegated |
done`, optionally `issue: gh:N`. The maintainer's role is decide-tasks and goal-setting;
everything else is yours to drive. Never invent tasks: new work enters the board through a
grilling/planning session or an explicit user request, and gets a row before it gets a branch.

## The loop (scheduler v2, maintainer-specified 2026-08-29)

1. **Read the board and compute the ready set.** Drop `done` rows and hard-blocked rows (any
   id in `needs` not `done`). Group what remains by soft-blockedness: the count of ids in
   `soft` that are not yet `done` (a `delegated` soft blocker still counts as un-done).
   Least soft-blocked category ranks first; `prio` (1 highest, default 3) sorts within a
   category; list position is only a tiebreak. The TOP FIVE of this ordering, builds and
   decides together, is the ready set. Soft order outweighs prio by construction, but among
   fully unblocked tasks prio alone decides.
2. **Deadlock check, before anything else.** Two shapes, both reported to the user
   immediately rather than worked around: a cycle in `needs` (topological sort fails), and
   exhaustion (todo entries remain but nothing is pickable and nothing is in flight). A
   third, operational one: a delegated session with no commits and no transcript activity
   for ~30 minutes -- go read its state (worktree diff, last transcript entry) and either
   finish its work by hand, relaunch it, or escalate; do not just wait.
3. **Fill the lanes: up to FIVE concurrent.**
   - `decide` entries in the ready set: queue for the user, batched into wizard/mail rounds
     where they carry code; they occupy attention, not a lane.
   - `build` entries: make sure a GitHub issue carries the spec (file one if the row has
     none), then delegate via the `enwiro-delegate` skill and set `status: delegated`; run
     on sonnet unless the row says otherwise. Three tiers (maintainer rule, 2026-08-30):
     `model: haiku` for mechanical work with an obvious done-state (renames, sweeps, small
     config/UI tweaks -- cheaper and faster beats smarter there), sonnet for regular build
     work, `model: fable` for design-heavy or cross-cutting builds. When boarding a row,
     assign the cheapest tier the task plausibly survives review on; the landing review is
     the safety net either way. Footprint conflicts are SOFT BLOCKER
     EDGES on the board (file-level -- a folder is not a footprint; that lesson cost a lane
     of parallelism once), not ad-hoc judgment: when a conflict is discovered at dispatch
     time, record the `soft` edge rather than just serializing silently. Picking a
     soft-blocked task while its blocker is in flight is allowed only when no cleaner task
     can fill the lane and the overlap is tolerable; otherwise leave the lane empty and say
     so in the report. Efficiency/process improvements are prio work by standing rule --
     schedule them ahead of ordinary rows so no time is spent working the old way.
4. **Monitor and land.** Watch delegated work (a cron tick per active delegation is enough);
   when a session finishes, run the `land-delegated-work` skill: suite, code-review,
   style-review, fix-or-file, merge locally. Then set the row `done`, commit the board change
   with the merge, and go to step 1.
5. **Report once per landing or decision-point,** per the standing protocol: what landed,
   what the reviews found, what is now unblocked, and which decide-tasks are waiting.
   No play-by-play.

## The periodic audit

Roughly every ten ticks (about every five hours of driving), run the full reconciliation --
the drift it catches is the kind each individual tick is blind to:

1. `git branch --no-merged main` -- any branch with commits main lacks that is not a live
   delegation is forgotten work (post-landing hook growth is the known producer); review and
   sweep it.
2. Open GitHub issues versus the board: every open issue maps to a row; every row's `issue:`
   field points at a real open (or deliberately open) issue; anything unmatched gets a row,
   a close, or a link.
3. Board statuses versus reality: every `delegated` row has a live worker; every `done` row
   has a merge on main; every `todo` row has a nameable gate (footprint, needs edge, or the
   user's decide queue). A status that cannot be justified is the finding.
4. Push distance, lingering sessions in landed worktrees, and env kanban status.

Report only the discrepancies and their root causes, and fix the mechanism (a skill edit, a
new check) rather than only the instance -- every audit finding so far became a rule.

## Board hygiene

- Review follow-ups become new rows (usually `build`, sometimes a `decide` + `build` pair
  when a finding needs a design call first), placed by priority judgment, linked to their
  filed issue.
- **Scope added to an in-flight issue never reaches its session** -- a session reads its
  issue once, at start (this lost the grill-directory scope once). New scope on dispatched
  work is either a re-brief (a continuation dispatch into the same env, which reads the
  comments fresh) or a follow-up row; commenting alone is not delivery.
- Reordering rows IS reprioritizing; do it when the user says so, or propose it in a report
  when the order has stopped matching reality.
- The board is committed like any other file (AGENTS.md rules apply). Keep rows terse; the
  linked issue and plans/*.md carry the detail.
