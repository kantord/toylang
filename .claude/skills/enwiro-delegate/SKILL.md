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

## 0. Board rows go to the worker pool first (gh:124, reshaped for opencode)

Board dispatches reuse the pool lanes -- enwiro envs `toylang@lane-1` .. `toylang@lane-5`,
cooked lazily with `enw prep`. The claude-era rationale (30-50k boot tokens) is moot at
DeepSeek prices; what still matters is the **worktree**: a reused lane keeps its cargo
`target/` cache, so compiles are minutes faster and disk stops multiplying per task.

Sessions are NOT reused across tasks: every dispatch is a fresh opencode session (a
lane's context ceiling, model matching, and `lane-context.py` eligibility checks were
claude-session machinery and no longer apply). A lane is **free** when its worktree is
clean, its last task's board row is archived, and no worker process is alive there
(`pgrep -x opencode` -- and during the transition `pgrep -x claude` -- with cwd under
the worktree). Never launch into a worktree with a live worker: the
two-sessions-one-worktree race predates this flow and survives it.

### Prepare the worktree (the coordinator does this before any pool dispatch)

    wt=$(enw prep 'toylang@lane-1')
    git -C "$wt" fetch origin
    git -C "$wt" switch -C issue-<N> origin/main

Only on a clean tree -- a dirty pool lane is not free, it is unlanded work. If branch
`issue-<N>` is already checked out in another worktree, that task already has an env;
a continuation dispatch there is what's wanted, not a pool lane.

## 1. Environment naming, for one-off delegations outside the pool

- `repo#<issue>` (github recipe) -- preferred; the worktree lands on branch issue-<N>.
  If no issue exists yet, create one first (`gh issue create`); public repo issues must
  be sanitized per the data-privacy rules.
- `repo@<branch-name>` (git recipe) -- for work with no issue; the brief must name the
  in-repo source-of-truth file explicitly.

## 1b. Push first -- worktrees branch from origin, not local main

If local main is ahead and unpushed, the worker builds against a stale base and its
branch merges back with semantic drift the suite only catches on main (this reverted
the anyhow work once). Before dispatching: push local main (standing authorization,
2026-08-29 -- ordinary pushes only, never force). Never dispatch onto a stale origin.

## 2. Launch the worker in kitty, then switch the user back

```sh
prev=$(i3-msg -t get_workspaces | jq -r '.[] | select(.focused).name')
enw activate 'toylang#12'    # or the pool lane env
enw wrap kitty 'toylang#12' -- --detach \
  /home/kantord/repos/toylang/.claude/scripts/opencode-worker.sh '<the brief>'
sleep 4   # let the window map on the env workspace; verify the worker is live
i3-msg "workspace \"$prev\"" >/dev/null
```

- `enw activate` yanks focus: capture the workspace BEFORE and switch back last. The
  sleep doubles as the moment to verify the opencode process is live (pgrep, cwd under
  the worktree).
- One model for all lanes during the rollout (the board's `model:` field is dormant for
  builds; `OPENCODE_MODEL` on the wrapper overrides per-dispatch if a ruling ever asks).

### The brief (opencode workers need more of it than claude workers did)

An opencode session reads AGENTS.md natively but gets NONE of the claude-side context:
no global CLAUDE.md rules (the issue-linked-branches rule does not fire), no hooks, no
memory. The brief carries everything, in this shape:

> You are a delegated worker for the toylang repository, in this git worktree on branch
> issue-<N>. FIRST read AGENTS.md at the worktree root and follow it throughout,
> including its commit rules. Your task is GitHub issue #<N>: run `gh issue view <N>`
> and read every comment before touching anything. [One or two sentences locating any
> in-repo source of truth.] Definition of done: [the concrete gates -- typically:
> implementation complete; `just test` green from the worktree root (a cold worktree
> compiles from scratch and is slow -- give it time, never abort it); no new clippy
> warnings in code you touched; work committed on this branch per AGENTS.md with the
> provenance line "Written by DeepSeek V4 Flash via opencode."]. Hard constraints: work
> ONLY inside this worktree; do NOT push; do not touch plans/. If a command is denied,
> adapt with an allowed alternative rather than retrying. If something needs deciding
> that this brief does not settle, file a GitHub issue for the coordinator with the
> alternatives, take the most conservative continuation, and keep going -- never stop
> to wait for a human. If you genuinely cannot complete the task, say exactly what
> blocked you instead of committing broken work.

## 2b. Update the board

Set the row's `status: delegated` (plus `lane: lane-<N>` for a pool dispatch -- the tick
scripts resolve the worktree through it), and keep the env navigable:

    enw goal set --env 'toylang@lane-1' 'gh:<N> <short title>'
    enw mark active --env 'toylang@lane-1'

A delegation without a board row means the task skipped planning -- add the row.

## 3. Steering a running worker

There is no SendMessage into an opencode worker. The steering primitive is: kill the
process, then resume the same session with a new message --

    opencode run --session <sessionID> -m "$OPENCODE_MODEL" '<correction / next step>'

(the sessionID is in the run's event log under `~/.cache/toylang-drive/opencode/`; a
resumed session keeps its full context -- verified 2026-08-30 on the gh:114 trial). A
worker whose process exited without committing is diagnosed from the same log: the last
events say what it was doing and whether a resume or a fresh dispatch is right.

## Cleanup

After the branch is reviewed and merged, `enw rm '<name>'` removes one-off envs (the
user's call). Pool lanes are never removed, only freed. Never remove an env with
unmerged work without explicit instruction.
