---
type: Playbook
name: enwiro-delegate
description: Delegate a task to a worker session in its own enwiro environment (git worktree) and workspace, visually navigable for the user. Use when the user wants implementation handed to a separate session/worktree/workspace, says "delegate this", "spawn a session for this", or names enwiro delegation.
---

# Delegate a task to a new enwiro environment

Workers run **DeepSeek V4 Flash** (or whatever `--model` names) talking directly to
OpenRouter, no CLI middleman, inside a disposable, fully-permissive microsandbox
microVM via `.claude/scripts/simple_dispatch.py` (2026-09-11 ruling: the only dispatch
mechanism anywhere in this project, replacing `sandbox_dispatch.py` and every
opencode-based path; see
[plans/simple-dispatch-design.md](../../../plans/simple-dispatch-design.md) for the
full design history, [plans/opencode-rollout.md](../../../plans/opencode-rollout.md)
for what it replaced). No permission allow-list to maintain: the sandbox is throwaway
and fully permissive by construction. Real incidents worth remembering go in
`plans/simple-dispatch-design.md`, not a new file.

The explicit enwiro variant below (a visible kitty window) remains for the rare one-off
the user wants visually navigable in the window manager; it is not what the autonomous
loop dispatches.

## 0. The default dispatch: a disposable sandbox, no enwiro at all (2026-09-11)

The one dispatch mechanism now:

    nohup python3 .claude/scripts/simple_dispatch.py <row-id> --brief-dir <dir-containing-row-id.txt> &

`--brief-dir` must contain a file named exactly `<row-id>.txt` -- not a `--brief <path>`
flag with an arbitrary filename. That runs the FULL cycle unsupervised in a disposable,
fully-permissive microsandbox microVM: real edits, its own real `just check` verify
with retries against the actual failure evidence, and a self-report if it gives up --
typically 5-20 minutes end to end, never waited on inline. It does NOT land on success
by itself (a deliberate design choice -- staying a pure dispatch primitive): check
`plans/dispatch-log.csv` for the row's final status once it exits, and on GREEN, run
`.claude/scripts/land-lane.sh land-patch <row-id> <patch-path>` yourself. Concurrency is
a single global batch, not a slot count: `simple_dispatch.py`'s own `--parallel` (default
3) fans multiple rows out INSIDE one call; check whether a dispatch is already running
with `.claude/scripts/dispatch-state.py --live` (real process cmdlines, not `msb list`'s
VM status, which stays "running" for a sandbox mid-teardown, and not board.yaml's
`status: delegated`, which can go stale on an escalated row) before launching another.
Orphaned sandboxes from an abruptly-killed dispatcher are reclaimed by
`dispatch-state.py --gc`, run automatically every drive tick -- no action needed for a
dispatch launched through the normal drive loop; run it by hand after a manually-killed
one-off dispatch.

No env, no workspace, no focus dance, no permission-wall boilerplate to teach around --
the sandbox is fully permissive by construction. A run that gives up (STUCK, RED,
TIMEOUT, SETUP_FAILED, FATAL) carries the model's OWN real-time explanation of what
blocked it, verbatim, in
`~/.cache/toylang-simple-dispatch/results/<row-id>-<run-id>-self-report.txt`
(`dispatch-state.py --status <row-id>` prints its path directly) -- read that first,
there is no transcript to reconstruct and no escalation composed automatically; decide
directly from what the agent already said (a narrower redispatch per its own
suggestion, or write the question into a `docs/.grill/` round yourself if it says this
isn't a scope problem at all).

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

Default: `simple_dispatch.py` (section 0). `--model` picks the build model (default
`deepseek/deepseek-v4-flash-0731`; the board's `model:` field is dormant for builds) --
there is no separate plan-decompose or critic phase, one continuous session drives the
whole attempt. It does NOT land on success itself -- run `land-lane.sh land-patch` by
hand once you see GREEN in `plans/dispatch-log.csv`.

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

### The brief: a plain task description, no boilerplate to wrap (2026-09-11)

`simple_dispatch.py` has no permission-wall boilerplate to teach around (the sandbox is
fully permissive by construction), so there is no `KNOWN DENIALS` list and no
`ESCALATION.md`-in-the-worktree convention to wrap the brief in either -- the file at
`<brief-dir>/<row-id>.txt` is just the task description, plain text, written directly:

- pointers to the in-repo source of truth (files, the ruling issue, existing patterns
  to read first);
- for a research task: exactly where to write findings (e.g. `plans/<name>.md`) and
  that it should be committed;
- any extra done-gates beyond `just check` passing (state the exact verify command with
  `--verify-cmd` if it isn't `just check`).

Two to eight sentences is normal; the worker reads it as its whole task, no
plan-decompose or critic phase wrapping it. AGENTS.md is read natively by the agent
inside the sandbox, same as before.

A blocker the dispatch itself cannot resolve is the model's OWN direct explanation of
why, written to `<row-id>-<run-id>-self-report.txt` in
`~/.cache/toylang-simple-dispatch/results/` -- read it yourself (`dispatch-state.py
--status <row-id>` prints the path); nothing composes a maintainer round for you
automatically for an ad-hoc dispatch outside the drive loop.

### Research dispatches: diagnosis is worker work too

A deep dive -- why a backend misbehaves, why a dispatch failed mid-task, what an odd
test failure means -- is DELEGATED, never done by the coordinator in its own session:
coordinator time is the expensive tier now, and reading a codebase is exactly what a
cheap worker does well. Same `simple_dispatch.py`, same brief shape as any other
dispatch. State the question plainly and where to write the answer:

> Board row `<row-id>`: this is a RESEARCH task, no compiler code changes expected.
> [the precise question, with every symptom already known -- failing command, error
> text, suspect files]. Investigate freely (read code, run `just check`, reproduce).
> Write findings to `plans/<name>.md` and commit it.

`simple_dispatch.py` runs its normal cycle against this brief and verifies+extracts the
committed findings file exactly like a code change (it still runs `--verify-cmd`, so
point it at something that actually passes once the findings file is committed, e.g.
`--verify-cmd "test -f plans/<name>.md"` if `just check` alone would not notice a
docs-only change). `--model` can lift a hard question to a stronger model per-dispatch.

## 2b. Update the board

Set the row's `status: delegated`. `dispatch-state.py` resolves everything from
`plans/dispatch-log.csv` and the row id directly -- there is no worktree to name or
resolve, and the `lane:` field is legacy, set on no new row. A delegation without a
board row means the task skipped planning -- add the row.

## 3. Steering a running dispatch

There is no SendMessage into a running dispatch. A run in progress cannot be
redirected: let it finish (GREEN, or a terminal STUCK/RED/TIMEOUT/SETUP_FAILED/FATAL)
rather than killing it mid-flight. Feedback on a failed verify attempt is already
automatic WITHIN one dispatch -- `agent_loop.py` feeds the real `just check` failure
back to the same in-process conversation on retry, up to `--retry-cap`, and if it gives
up, its own self-report is fed into the very next retry attempt's instructions too (the
built-in self-healing loop -- see `plans/simple-dispatch-design.md`'s "Course
correction" section).

Once a dispatch has fully exited, TWO ways to continue it exist, unlike the old
opencode-based pipeline (which was stateless per attempt with no resume at all):
- **Fresh dispatch, sharper brief** -- the default. Read the self-report, write a
  narrower or more specific brief, dispatch again as a new row/run.
- **`--resume-from`/`--resume-patch`** -- continue the EXACT SAME session (its full
  conversation, and optionally its prior patch reapplied via `git am`) with a new,
  narrower instruction, while the provider-side prompt cache might still be warm. Manual
  only, single row: `simple_dispatch.py <row-id> --brief-dir <dir-with-new-instruction>
  --resume-from <results-dir>/<row-id>-<run-id>-messages.json --original-task-file
  <the-original-brief> [--resume-patch <results-dir>/<row-id>-<run-id>.patch]`. Worth it
  shortly after the original run ended (before the cache goes cold); if it's been a
  while, a fresh dispatch costs about the same anyway.

## Cleanup

A landed `toylang-lanes` worktree is removed by the coordinator at landing
(`git worktree remove`, from the land skill) -- the branch and commits live in the main
repo's .git, so nothing is lost and disk stays flat. Enwiro envs stay the user's to
remove (`enw rm`), never with unmerged work without explicit instruction.
