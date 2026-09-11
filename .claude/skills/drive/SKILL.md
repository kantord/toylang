---
type: Playbook
name: drive
description: Drive development autonomously from plans/board.yaml - the ordered task board with dependencies. Use when the user says "drive", "keep going", "work the board", asks what's next, or wants autonomous development to continue while they only do grilling and goal-setting.
---

# Drive the board

## How ticks arrive (since 2026-08-30: the drive loop, not session crons)

Orchestration is externally scheduled: the maintainer starts `just drive` (runs
`.claude/scripts/drive_loop.py` via `uv run`) by hand and stops it by killing the
process. The loop fires `.claude/scripts/drive_tick.py` every DRIVE_INTERVAL seconds
(also `uv run`-invoked, 2026-09-11: the whole `.claude/scripts/` tree is a proper
`uv`-managed Python project now, no shell scripts left) -- each tick a fresh,
standalone `claude -p` request in auto permission mode (no cross-tick --resume, dropped
2026-08-31: it saved well under a cent/tick and both of that night's flakiest ticks
happened on a resumed session). The script also picks the model (sonnet routinely, fable
when a lane looks landable) and revives the dev server after a reboot.

Editing this skill, any other skill, or the tick scripts takes effect on the very next
tick automatically -- there is no cached session to drop.

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
   `docs/.grill/` process immediately. An inbox record whose `page` is a
   `docs/.grill/*.round.yaml` is a wizard SUBMISSION -- an explicit click, applied at once,
   no quiet period (the quiet rule protects half-typed compose notes, not button presses).
   A record whose `page` is a `docs/.grill/*.forest.yaml` (grill-via-annotations skill,
   "Forest rounds") is the same kind of explicit-click submission -- process immediately, and
   processing means writing the answer into the node (`status: answered`, the `answer` block)
   FIRST, then clearing the inbox record second: the inbox isn't durable, so that order is what
   makes a mid-tick death reprocess safely instead of losing the answer.
   Grilling runs CONCURRENTLY with build work: keep TWO rounds buffered in `docs/.grill/`
   whenever ready decides remain (compose the second before the first is answered, never
   duplicating a pending round's questions), so the maintainer can answer back-to-back
   while workers grind. A record whose `page` is a `plans/*.md` file is a
   plan decision: an explicit click, applied at once, no quiet period ("Plan approval"
   below). Clearing at capture is RE-READ, FILTER BY ID, WRITE -- one atomic step, printing
   what is removed. Never empty an array wholesale from a stale read: the maintainer keeps
   composing while a tick works, and a blanket `composed = []` deleted an unread 14:17 note
   on 2026-08-30 with no recovery path (the endpoint keeps no log).
3. Verify push distance before any dispatch (worktrees branch from origin).

## Stall diagnosis, learned the hard way

Superseded, 2026-09-11: the diagnosis below (worktree mtimes, ESCALATION.md, opencode
event logs) described the old sandbox_dispatch.py/opencode pipeline, kept here only as
history. Under simple_dispatch.py there is no worktree per dispatch, no ESCALATION.md
channel, and no opencode event log to read -- a dispatch reports through exactly two
plain surfaces: `plans/dispatch-log.csv` (one row per run: status, cost, patch path) and
`~/.cache/toylang-simple-dispatch/results/<row>-<run_id>-*` files (the extracted patch,
the full agent log, and -- the real diagnosis surface -- `-self-report.txt`: the model's
own direct explanation of what blocked it, asked for in-context at the moment it gave
up, not reconstructed afterward from a transcript). `.claude/scripts/dispatch_state.py`
reads both surfaces for you; a tick's own trigger text already carries the self-report
verbatim for any STUCK/RED/TIMEOUT/SETUP_FAILED/FATAL row -- read that directly instead
of digging through logs. See `plans/simple-dispatch-design.md` for the full design
history and why this replaced a separate post-hoc reviewer entirely.

Historical record of the retired pipeline's diagnosis, preserved for context: the
dead-worker signature (claude-era lanes) was the newest file in the session's
tool-results dir being its own session-start hook message -- the worker died (usually
machine suspend) and a fresh idle session auto-spawned. An opencode worker's process
exiting was its turn ending -- there was no idle session left behind; a committed
`ESCALATION.md` was the worker's channel for decisions its brief did not settle, and the
event log (`~/.cache/toylang-drive/opencode/*-<lane>.jsonl`) was the diagnosis source
when nothing else said what a worker was doing.

**The coordinator is a router (maintainer direction, 2026-08-30).** The asymptote every
change moves toward: a tick spends its turns on DECISIONS -- what to dispatch, what to
land, what to surface to the maintainer -- executed through the four mechanical
surfaces (simple_dispatch.py, land_lane.py, board-archive.py, round files), and reads
results rather than exploring. sandbox_dispatch.py, dispatch-worker.sh, and every
opencode-based worker are retired (2026-09-11 ruling: simple_dispatch.py + agent_loop.py
is the only dispatch mechanism) -- never invoke any of them. The gate script hands each
tick a pre-computed state snapshot in the prompt: act on it instead of re-reading the
board, re-checking lanes, and re-polling stores; re-verify only what you are about to
modify. The deliberate exception, for now, is the landing diff read -- that judgment
stays in-tick until the rollout review prices a cheap-model alternative.

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
decide-tasks and goal-setting; everything else is yours to drive. Never invent tasks while
real work remains: new work enters the board through a grilling/planning session or an
explicit user request, and gets a row before it gets a branch.

**The idle exception (maintainer ruling, 2026-08-30).** When the board runs dry -- no ready
builds, lanes empty -- the coordinator MAY self-originate, two ways, both with visible
provenance ("self-originated, idle board" on the row/issue):

- Board ONE OR TWO exploration ideas and work them: research-shaped rows (spikes, audits,
  benchmarks, tooling probes) that are cheap and reversible. Anything touching language
  design or user-facing semantics is NOT self-buildable -- it becomes a decide row and a
  round question instead.
- Problems the coordinator discovered itself (a flaky test, a doc contradiction, a cost
  anomaly in lanes.csv, a suspicious pattern in a backend) go to the maintainer's inbox:
  a decide row plus an `escalation`-flow question in the next wizard round, with the
  evidence attached. Discovery is welcome any time; SURFACING it always beats silently
  acting on it.

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
3. **Dispatch is a single batched call, not a per-row slot pool** (2026-09-11 ruling:
   simple_dispatch.py's own `ThreadPoolExecutor` fan-out (`--parallel`, default cap 3)
   IS the concurrency -- one process handles up to 3 rows at once and only exits once
   every row in that batch has a final status. "Occupied" is therefore binary, not a
   slot count: `dispatch_state.py --live` reads real process cmdlines directly, never
   board.yaml's `status: delegated` (which can go stale on an escalated row exactly the
   way it already did under the old model) or `msb list`'s VM status (which also shows
   sandboxes mid-teardown). Never launch a second batch while one is already live. When
   several ready rows share a file footprint (the draft.md migration family, say),
   dispatch ONE of the family per batch and record the `soft` edges between the rest --
   parallel same-file dispatches just manufacture merge conflicts.
   - `decide` entries in the ready set: queue for the user, batched into wizard/mail rounds
     where they carry code; they occupy attention, not a dispatch slot.
   - `build` entries: make sure a GitHub issue carries the spec (file one if the row has
     none), write a brief per the enwiro-delegate skill to `plans/simple-briefs/ROW-ID.txt`
     (this exact filename -- simple_dispatch.py requires `--brief-dir`/`<row_id>.txt`),
     then dispatch a batch of up to 3 ready rows in ONE call, DETACHED --
     `nohup uv run --project .claude/scripts .claude/scripts/simple_dispatch.py
     ROW-ID-1 ROW-ID-2 ROW-ID-3 --brief-dir plans/simple-briefs --parallel 3 &` -- and set each row's `status:
     delegated` in the same commit as writing its brief. Every dispatch runs FULLY
     unsupervised end to end: real edits, its own `just check` verify with retries, a
     self-report if it gives up, and a real extracted patch on any outcome that made
     edits -- but it does NOT self-land (a deliberate design choice, staying a pure
     dispatch primitive; see `plans/simple-dispatch-design.md`). Landing a GREEN result
     is the tick's own job: `uv run --project .claude/scripts .claude/scripts/land_lane.py
     land-patch ROW-ID PATCH-PATH`, DETACHED, same
     as any other landing (duty 4 in "Monitor and land" below). A non-GREEN outcome
     (STUCK, RED, TIMEOUT, SETUP_FAILED, FATAL) carries the agent's OWN real-time
     explanation of what blocked it, verbatim, already surfaced in the tick's trigger
     text (`dispatch_state.py --status ROW-ID`, or read
     `~/.cache/toylang-simple-dispatch/results/ROW-ID-*-self-report.txt` directly) --
     there is no transcript to reconstruct and no separate escalation-composition step;
     decide directly from what the agent already said: a narrower redispatch per its own
     suggestion, or a decide-row escalation if it says this isn't a scope problem at all.
     Record a genuinely surprising incident (a wrong self-report, a repeated failure
     shape, a real cost anomaly) as a note in `plans/simple-dispatch-design.md`, not a
     new file. Footprint conflicts are SOFT BLOCKER EDGES on the board (file-level -- a
     folder is not a footprint; that lesson cost a lane of parallelism once), not ad-hoc
     judgment: when a conflict is discovered at dispatch time, record the `soft` edge
     rather than just serializing silently. Picking a soft-blocked task while its
     blocker is in flight is allowed only when no cleaner task can fill the slot and the
     overlap is tolerable; otherwise leave the slot empty and say so in the report.
     Efficiency/process improvements are prio work by standing rule -- schedule them
     ahead of ordinary rows so no time is spent working the old way.
4. **Monitor and land.** A dispatched batch reports a final status per row when it exits
   (`dispatch_state.py --status ROW-ID`); a GREEN row lands via `uv run --project
   .claude/scripts .claude/scripts/land_lane.py land-patch ROW-ID PATCH-PATH` DETACHED --
   the gate (full `just test` in a throwaway worktree) is the WHOLE pre-merge check,
   deterministic, no model reads the diff before merging (see land_lane.py's own header). Move the landed row to `plans/board-archive.yaml` with
   `status: done` (issue #113: never flip it in place), commit the board change with the
   merge, and go to step 1. Post-land review (reading the new commit's diff for real
   follow-up problems, filing rows for them) happens AFTER landing, asynchronously, per
   duty (c) in step 3 above -- never a pre-merge gate.
   (`land-delegated-work` is a DIFFERENT skill, for enwiro-delegate research/interactive
   sessions, not board-driven build dispatches -- do not conflate the two.)
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
