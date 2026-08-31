---
type: Playbook
name: enwiro-delegate
description: Delegate a task to a worker session in its own enwiro environment (git worktree) and workspace, visually navigable for the user. Use when the user wants implementation handed to a separate session/worktree/workspace, says "delegate this", "spawn a session for this", or names enwiro delegation.
---

# Delegate a task to a new enwiro environment

Workers run **opencode + DeepSeek V4 Flash** via `.claude/scripts/opencode-worker.sh`
(maintainer ruling, 2026-08-30: claude-code delegation is RETIRED -- no new delegated
work on claude code until the re-evaluation gate in
[plans/opencode-rollout.md](../../../plans/opencode-rollout.md), roughly 30 landed
lanes in). The wrapper never passes `--auto`: the maintainer's opencode.jsonc
allow-list (deny-by-default, chezmoi-managed) is the permission guardrail, and the
wrapper writes the lanes.csv telemetry row on exit. Every rollout incident gets
recorded in the rollout log -- that observability is part of this ruling, not optional.

The kitty window shows the live colorized event stream (opencode-peek.py), so lanes
stay visually navigable in the window manager exactly as before.

## 0. The default dispatch: no enwiro at all (maintainer simplification, 2026-08-30)

A lane is just a git worktree plus a background worker process:

    .claude/scripts/dispatch-worker.sh <issue-number> '<the brief, shaped per section 2>'

That creates (or continues) `~/.local/share/toylang-lanes/issue-<N>` on branch
`issue-<N>` cut from the largest live `to-merge-*` accumulator (origin/main when
none exists), refuses if a live worker already owns the worktree
(the two-sessions-one-worktree race predates this flow and survives it), launches the
worker detached, and prints the `tail -f` line for watching the live colorized stream.
No env, no workspace, no focus dance. The worker-pool machinery (gh:124: `enw prep`
lanes, `lane-context.py`, the board's `lane:` field) is retired for new dispatches --
sccache makes cold worktrees cheap, so worktree reuse stopped paying for its
complexity. Every dispatch is a fresh opencode session on a fresh-or-continued
worktree.

The full enwiro flow below (env + workspace + kitty window) remains available for the
rare one-off the user explicitly wants visually navigable in the window manager; a
`gh issue create` first if the task has no issue (public repo issues sanitized per the
data-privacy rules).

## 1b. Push first -- worktrees branch from origin, not local main

If local main is ahead and unpushed, the worker builds against a stale base and its
branch merges back with semantic drift the suite only catches on main (this reverted
the anyhow work once). Before dispatching: push local main (standing authorization,
2026-08-29 -- ordinary pushes only, never force). Never dispatch onto a stale origin.

## 2. Launch

Default: `dispatch-worker.sh` (section 0). One model for all lanes during the rollout
(the board's `model:` field is dormant for builds; `OPENCODE_MODEL` overrides
per-dispatch if a ruling ever asks). On worker exit the wrapper fires a drive tick
itself -- event-driven landing, no quiet window -- so a finished lane lands within
minutes, not tick-intervals.

For the explicitly-requested enwiro variant only:

```sh
prev=$(i3-msg -t get_workspaces | jq -r '.[] | select(.focused).name')
enw activate 'toylang#12'
enw wrap kitty 'toylang#12' -- --detach \
  /home/kantord/repos/toylang/.claude/scripts/opencode-worker.sh '<the brief>'
sleep 4   # let the window map on the env workspace; verify the worker is live
i3-msg "workspace \"$prev\"" >/dev/null
```

(`enw activate` yanks focus: capture the workspace BEFORE and switch back last.)

### The brief: dispatch-worker.sh wraps the boilerplate (2026-08-30)

An opencode session reads AGENTS.md natively but gets NONE of the claude-side context;
the standard build boilerplate (role, AGENTS.md, gates, hard constraints, KNOWN
DENIALS, ESCALATION.md protocol) lives IN dispatch-worker.sh and is wrapped around
whatever you pass. So the dispatcher writes ONLY the task-specific middle:

- pointers to the in-repo source of truth (files, the ruling issue, existing patterns
  to read first);
- for a continuation after a failed run: what killed the last run (from its event
  log) and the concrete adaptation, stated plainly at the top;
- any extra done-gates beyond the standard ones.

Two to five sentences. Do not restate the boilerplate -- it is added verbatim by the
script (read it there when editing it; new denial classes are added there once, not
per-brief). `BRIEF_RAW=1 dispatch-worker.sh ...` passes the brief through unwrapped
for shapes the boilerplate does not fit (research dispatches below).

(Escalation is a committed file, not a GitHub issue: workers have no `gh issue create`
permission by design -- rollout incident #1, 2026-08-30 -- and a file on the branch is
what the coordinator's event-driven tick reads anyway. The coordinator turns it into
the real issue/board row and removes it before merging; ESCALATION.md never lands on
main.)

### Research dispatches (2026-08-30): diagnosis is worker work too

A deep dive -- why a backend misbehaves, why a lane died mid-task, what an odd test
failure means -- is DELEGATED, never done by the coordinator in its own session:
coordinator time is the expensive tier now, and reading a codebase is exactly what a
cheap worker does well. Same `dispatch-worker.sh` with `BRIEF_RAW=1` (the build
boilerplate does not fit), usually as a continuation into the lane that raised the
question; the brief is this shape in full:

> You are a research worker for the toylang repository, in this git worktree. FIRST
> read AGENTS.md. Your task is to ANSWER A QUESTION, not to fix anything: [the precise
> question, with every symptom the dispatcher already has -- failing command, error
> text, suspect files]. Investigate freely (read code, run `just check`, reproduce);
> do NOT change or discard existing working-tree edits beyond reverting your own
> experiments. Deliverable: RESEARCH.md at the worktree root -- the answer, the
> evidence, and a recommendation (fix shape, or what to escalate) -- COMMITTED on this
> branch. That commit is your entire output; never stop to wait for a human.

The worker's exit fires the event tick as always; that tick reads RESEARCH.md, acts on
it (follow-up issue, informed continuation brief, board row), and strips the file
before any merge -- like ESCALATION.md, it never lands on main. `OPENCODE_MODEL` can
lift a hard question to a stronger cheap model per-dispatch.

## 2b. Update the board

Set the row's `status: delegated`. The tick scripts resolve the worktree from the
issue number (`~/.local/share/toylang-lanes/issue-<N>` first, the legacy enwiro base as
fallback); the `lane:` field is legacy and set on no new row. A delegation without a
board row means the task skipped planning -- add the row.

## 3. Steering a running worker

There is no SendMessage into an opencode worker. The steering primitive is: kill the
process, then resume the same session with a new message --

    opencode run --session <sessionID> -m "$OPENCODE_MODEL" '<correction / next step>'

(the sessionID is in the run's event log under `~/.cache/toylang-drive/opencode/`; a
resumed session keeps its full context -- verified 2026-08-30 on the gh:114 trial). A
worker whose process exited without committing is diagnosed from the same log: the last
events say what it was doing and whether a resume or a fresh dispatch is right.

## Cleanup

A landed `toylang-lanes` worktree is removed by the coordinator at landing
(`git worktree remove`, from the land skill) -- the branch and commits live in the main
repo's .git, so nothing is lost and disk stays flat. Enwiro envs stay the user's to
remove (`enw rm`), never with unmerged work without explicit instruction.
