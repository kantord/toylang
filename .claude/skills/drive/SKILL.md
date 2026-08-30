---
type: Playbook
name: drive
description: Drive development autonomously from plans/board.yaml - the ordered task board with dependencies. Use when the user says "drive", "keep going", "work the board", asks what's next, or wants autonomous development to continue while they only do grilling and goal-setting.
---

# Drive the board

## How ticks arrive (since 2026-08-30: the drive loop, not session crons)

Orchestration is externally scheduled: the maintainer starts
`.claude/scripts/drive-loop.sh` by hand (and stops it by killing the process). The loop
fires `.claude/scripts/drive-tick.sh` every DRIVE_INTERVAL seconds -- each tick a
`claude -p` request in auto permission mode that resumes ONE coordinator session; the
script watches that session's context from outside and starts a fresh one past
MAX_CONTEXT. The script also picks the model (sonnet routinely, fable when a lane looks
landable) and revives the dev server after a reboot.

After editing this skill, any other skill, or the tick scripts: `rm ~/.cache/toylang-drive/session-id`
so the NEXT tick boots a fresh session that reads the updated files -- a resumed session keeps
serving its stale cached copy. Never kill a mid-tick process for this; the drop takes effect at
the next tick boundary on its own.

A tick session therefore NEVER arms crons, background watchers, or its own follow-up
wake-ups -- scheduling belongs to the loop script. And a tick NEVER backgrounds work it
must act on before ending: a `-p` session that ends its turn is OVER -- no notification
ever reaches it. The post-merge main suite runs FOREGROUND, and the push happens in the
same tick as the merge (a tick that exited with main ahead of origin dropped a landing
once, 2026-08-30: the backgrounded suite died with the session and the push never fired).
Every tick:

1. **Reconstruct in-flight reality before acting**: for every `delegated` board row, check
   its worktree (commits vs main, dirty files, live worker via pgrep cwd). Trust disk over
   anything remembered from earlier ticks -- the maintainer or another session may have
   acted in between. Adopt healthy lanes, intervene on dead ones (see the stall guidance
   below), land finished ones.
2. Poll `docs/.annotations/inbox.json` AND `docs/.annotations/notes.json` (compose messages
   and span notes), applying entries 5+ minutes quiet or marked read; wizard rounds in
   `docs/.grill/` process immediately. A record whose `page` is a `plans/*.md` file is a
   plan decision: an explicit click, applied at once, no quiet period ("Plan approval"
   below). Clearing at capture is RE-READ, FILTER BY ID, WRITE -- one atomic step, printing
   what is removed. Never empty an array wholesale from a stale read: the maintainer keeps
   composing while a tick works, and a blanket `composed = []` deleted an unread 14:17 note
   on 2026-08-30 with no recovery path (the endpoint keeps no log).
3. Verify push distance before any dispatch (worktrees branch from origin).

## Stall diagnosis, learned the hard way

The dead-worker signature (claude-era lanes, still live during the rollout transition):
the newest file in the session's tool-results dir is its own session-start hook message --
the worker died (usually machine suspend) and a fresh idle session auto-spawned. A worktree
whose tree is fully STAGED by a dead worker may be verified (suite + build) and committed by
the coordinator directly, with the commit message saying so; uncommitted half-done work gets
a continuation dispatch into the same env whose brief says to read ALL issue comments and
assess the existing diff. File-write mtimes and commit times are the truth; transcript
timestamps lie.

An opencode worker's process exiting IS its turn ending -- there is no idle session left
behind. FIRST check the worktree for a committed `ESCALATION.md`: that is the worker's
designed channel for decisions its brief did not settle (workers cannot file GitHub
issues by permission design) -- turn it into the real issue/board row/decide entry, and
remove the file from the branch before any merge; it never lands on main. Otherwise
diagnose from the event log (`~/.cache/toylang-drive/opencode/*-<lane>.jsonl`): the
last events say what it was doing. Uncommitted work continues via
`opencode run --session <sessionID>` with a correction message (full context retained);
anything that smells like a worker-quality problem gets a row in
plans/opencode-rollout.md's incident table -- that log is the rollout's evidence base.

The tick's diagnosis budget is the event log, git state, and the suite output --
ROUTING evidence. The moment understanding requires reading source files or
experimenting (why a backend misbehaves, what a generated program actually does), the
tick STOPS and dispatches a research worker instead (enwiro-delegate skill, "Research
dispatches"): coordinator minutes cost more than whole worker lanes now, and the
research worker's exit brings the answer back through the normal event tick. A tick
that catches itself grepping src/ has already gone too far.

Board-editing scripts match a row id ONLY with its terminator -- `'- id: <slug>\n'`, never a
bare prefix: `- id: nullary-functions` also matches `nullary-functions-decision`, and the
falls-through `index('status: todo')` then flips whatever row comes next (it silently marked
a decide row delegated once; the audit caught it, not the edit). That rule still governs a
landing flip (issue #113): the matched row is cut from `plans/board.yaml` and appended, whole,
to `plans/board-archive.yaml` with `status: done` -- never edited to `done` in place.

A research task with big results gets SPLIT into per-item follow-up rows at capture time --
never one mega review row that sits unfinished (maintainer rule, 2026-08-30; the oddities
inventory proved it: most of its 16 items got settled piecemeal while the mega row aged).
That split happens when the plan is APPROVED, not when it is written -- see "Plan approval"
below.

Two bookkeeping rules the audits keep re-finding: a follow-up issue filed during a landing
gets its board row IN THE SAME COMMIT (an issue without a row is invisible to this loop --
four accumulated once); and an inbox record dismissed as stale gets that dismissal NAMED in
the tick's report (a silent no-op clear is the one path where maintainer input can vanish
without a trace).

`plans/board.yaml` is the single source of truth for live work: an ordered list where position
is priority. Each entry: `id`, `title`, `kind: build | decide`, `needs: [ids]`, `status: todo |
delegated`, optionally `issue: gh:N`. Landed rows do not stay here (issue #113): they move to
`plans/board-archive.yaml`, same schema, `status: done`, append-only, kept for provenance only.
A `needs`/`soft` id not found in the live board is satisfied -- it landed and was archived; the
archive is never consulted to decide whether something is blocked. The maintainer's role is
decide-tasks and goal-setting; everything else is yours to drive. Never invent tasks: new work
enters the board through a grilling/planning session or an explicit user request, and gets a
row before it gets a branch.

## The loop (scheduler v2, maintainer-specified 2026-08-29)

1. **Read the board and compute the ready set.** No `done` rows to drop -- they live in
   `plans/board-archive.yaml` (issue #113). Drop hard-blocked rows instead (any id in `needs`
   that is still present in the live board, i.e. still `todo` or `delegated`; an id absent from
   the live board is satisfied). Group what remains by soft-blockedness: the count of ids in
   `soft` still present on the live board (a `delegated` soft blocker still counts as un-done;
   an absent one does not). Least soft-blocked category ranks first; `prio` (1 highest, default
   3) sorts within a category; list position is only a tiebreak. The ready set is TWO
   queues, not one (maintainer fix, 2026-08-30 -- decides were crowding builds out of a
   shared top-five, leaving lanes empty): ALL ready `decide` rows queue for the
   maintainer's grill/mail rounds, and the top ready `build` rows fill the free lanes
   up to the cap. Soft order outweighs prio by construction, but among fully unblocked
   tasks prio alone decides.
2. **Deadlock check, before anything else.** Two shapes, both reported to the user
   immediately rather than worked around: a cycle in `needs` (topological sort fails), and
   exhaustion (todo entries remain but nothing is pickable and nothing is in flight). A
   third, operational one: a delegated session with no commits and no transcript activity
   for ~30 minutes -- go read its state (worktree diff, last transcript entry) and either
   finish its work by hand, relaunch it, or escalate; do not just wait.
3. **Fill the lanes: up to EIGHT concurrent** (raised from five 2026-08-30; cheap
   workers moved the constraint to landing throughput, disjoint footprints, and local
   CPU). When several ready rows share a file footprint (the draft.md migration family,
   say), dispatch ONE of the family per cycle and record the `soft` edges between the
   rest -- parallel same-file lanes just manufacture merge conflicts.
   - `decide` entries in the ready set: queue for the user, batched into wizard/mail rounds
     where they carry code; they occupy attention, not a lane.
   - `build` entries: make sure a GitHub issue carries the spec (file one if the row has
     none), then dispatch with `.claude/scripts/dispatch-worker.sh <N> '<brief>'` (the
     enwiro-free default -- brief shape in the enwiro-delegate skill, section 2) and set
     `status: delegated`; the worker-pool and per-issue enwiro flows are legacy, kept
     only for their in-flight lanes. ALL delegated builds run
     opencode + DeepSeek V4 Flash through `.claude/scripts/opencode-worker.sh`
     (maintainer ruling, 2026-08-30: claude-code delegation is retired -- no new
     delegated work on claude code, no exceptions for tier or size, until the
     re-evaluation gate in plans/opencode-rollout.md at ~30 landed opencode lanes).
     The board's model tiers are dormant for builds during the rollout. Two standing
     obligations travel with the ruling: EVERY rollout incident (retry, stall, review
     finding traceable to the worker, abandoned lane) gets a row in the rollout log's
     incident table, and when the landed-lane count reaches ~30 the coordinator boards
     the `opencode-rollout-review` decide row. The landing review is
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
   style-review, fix-or-file, merge locally. Then move the row to `plans/board-archive.yaml`
   with `status: done` (issue #113: never flip it in place), commit the board change with the
   merge, and go to step 1.
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

## Plan approval

Research and planning output lands as `plans/<name>.md` carrying YAML frontmatter, and the
maintainer rules on it in the mail app rather than in the terminal (kantord/toylang#110):

```yaml
---
status: proposed        # proposed | approved | needs-changes
issue: gh:104           # the issue that commissioned it, when one did
---
```

A `proposed` plan is an inbox item in the mail app's "Plan approvals" folder, rendered in full,
with Approve and Needs changes under it and a notes box; the board's plans panel shows where
every statused plan stands. The maintainer's other channel is the file itself -- a plan is a
committed markdown document, so changes they want made are written straight into it, and the
notes box carries what an edit cannot say.

The click posts ONE record to `docs/.annotations/inbox.json`: `page` is the plan's path, `block`
is 0, and `edited` is `{"decision": "approve" | "needs-changes", "notes": ...}`. Applying it:

- Re-read the plan first. The maintainer may have edited it, and their edits outrank the notes.
- Rewrite the frontmatter `status`, commit that with whatever the decision produced, and clear
  the record.
- **Approve** means the plan is ready to become build work, not that it is one row. Split it
  into per-item rows the same way a big research result is split, and link each to its issue.
- **Needs changes** means another planning phase: a follow-up row or a re-brief into the same
  environment, carrying the notes and the maintainer's edits.

Only a plan that declares a status is in the flow at all. Most of `plans/` predates this and is
historical record; back-filling a status onto a document nobody actually ruled on would be
inventing the ruling.

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
