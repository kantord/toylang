# Worker pool: persistent lanes, sessions reused under a token ceiling

Design record for gh:124. The operative protocol lives in the enwiro-delegate skill
(section 0), the landing side in land-delegated-work; this file records why the open
design points settled the way they did, and what was actually measured on 2026-08-30.

The idea (maintainer, 2026-08-30): a fresh enwiro env plus a cold claude session per
task pays the boot -- system prompt, CLAUDE.md, AGENTS.md, repo orientation -- every
time. Keep a small pool of lanes instead; after a task lands, the same session takes
the next board row, branching fresh from main in the same worktree, until its context
crosses a ceiling.

## What was measured before settling anything

- A bare interactive haiku session in an empty directory boots to 28,939 context
  tokens before its first real turn. In a toylang worktree, with the issue and
  AGENTS.md read, boot lands in the 30-50k range. That is the per-task overhead the
  pool amortizes, and it recurs on every one of the ~5 concurrent lanes.
- `lanes.csv` (the gh:123 SessionEnd hook) held exactly one row when this was built:
  the coordinator session, peak context 248,580 over 320 turns. No worker rows exist
  yet, so nothing about worker task sizes could be derived from telemetry. The token
  ceiling below is therefore inherited, not measured.
- Both dispatch channels were verified live: an idle interactive session processed a
  SendMessage as a new turn, and `claude -p --resume <sid>` of an exited interactive
  session answered a question only its prior context could answer.
- Transcript availability splits by harness generation. `claude -p` sessions and
  older-format interactive sessions write `~/.claude/projects/<munged-cwd>/<sid>.jsonl`
  live; interactive sessions on the newer harness (every toylang lane session observed
  today, this one included) expose no transcript file at all while running or after.
  Any design that assumes the coordinator can always read a lane's context is wrong
  today.

## The settled points

### Reuse is an optimization with a safe fallback at every step

This is the principle the measurement variance forced. The pool never depends on a
fact the coordinator might not be able to observe: an unreadable context falls back to
a task-count backstop, a wrong session id degrades to an effectively cold resume that
still carries a complete brief, a live-but-ineligible lane is simply not dispatched
to. Getting a reuse decision wrong costs the warm-boot saving, never correctness.

### Env and branch naming

Lanes are git-recipe envs `toylang@lane-1` .. `toylang@lane-5` (the concurrency cap),
cooked lazily. The task branch inside is always `issue-<N>`, force-cut from
origin/main by the coordinator before dispatch, so per-task branches, merges, and the
`main..HEAD` checks all work unchanged. Navigability, which the per-issue env name
used to provide, moves to `enw goal set` (updated at dispatch and landing) and to the
`lane:` field on the board row. The alternative -- renaming envs per task -- has no
enwiro support and would break the stable symlink the tick scripts resolve worktrees
through.

### How dispatch reaches a live session

SendMessage when the worker process is alive; kitty relaunch with `claude --resume
<sid>` when it is gone; a plain fresh launch when recycling or cold. Both warm paths
were verified (above). A queued-prompt file was rejected: nothing existing consumes
it, and SendMessage already delivers into a live session's turn loop with no new
moving parts.

### Provenance across tasks in one session

Unchanged by construction: provenance is recorded per commit on a per-task branch, so
a session spanning tasks records nothing differently. The one real leak channel is
residual context -- task A's decisions silently shaping task B -- so the reuse brief
carries a mandatory hygiene line ("your previous task is landed history; re-derive
everything from the issue"). `lanes.csv` rows aggregate a whole session, which now
spans tasks; per-task cost lives in `dispatches.csv` (one row per dispatch, context
at dispatch time), whose consecutive deltas per session id are per-task growth.

### When a lane is force-recycled

Recycle (fresh session, same worktree -- the cargo target/ cache survives) on any of:

- context at or past MAX_LANE_CONTEXT = 90,000 -- the same number drive-tick.sh
  enforces on the coordinator, adopted for consistency, not measured for workers;
- context unreadable and the session already took 3 tasks (`dispatches.csv` count) --
  the backstop that keeps reuse alive on the newer harness at all;
- model tier mismatch with the row (haiku work must not burn a fable session's lane,
  and a fable row must not inherit haiku-grade context);
- the previous task needed a stall intervention.

Never with a live worker process in the worktree (the two-sessions-one-worktree race);
a live ineligible lane just waits. `enw rm` on a lane stays a human decision.

## To revisit once telemetry accumulates

Both constants are placeholders standing on one coordinator data point. When
`lanes.csv` has a week of worker rows and `dispatches.csv` a few reused sessions:
per-task context growth is the delta series in `dispatches.csv`; the ceiling should
become roughly (model window minus observed p90 task growth), and the task-count
backstop should be re-derived from the same numbers. If the newer harness starts
exposing transcripts (or a context probe), the backstop can go entirely.
