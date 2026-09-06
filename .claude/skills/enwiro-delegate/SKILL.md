---
type: Playbook
name: enwiro-delegate
description: Delegate a task to a worker session in its own enwiro environment (git worktree) and workspace, visually navigable for the user. Use when the user wants implementation handed to a separate session/worktree/workspace, says "delegate this", "spawn a session for this", or names enwiro delegation.
---

# Delegate a task to a new enwiro environment

Workers run **opencode + DeepSeek V4 Flash / GLM** inside a disposable, fully-permissive
microsandbox microVM via `.claude/scripts/sandbox_dispatch.py` (kanban ruling,
2026-09-06 -- `dispatch-worker.sh` and claude-code delegation are both retired; see
[plans/opencode-rollout.md](../../../plans/opencode-rollout.md) for the full history).
No permission allow-list to maintain: the sandbox is throwaway and fully permissive by
construction, which is the whole reason this superseded the plain-worktree path rather
than extending its `KNOWN DENIALS` list further. Every rollout incident still gets
recorded in the rollout log -- that observability is not optional.

The explicit enwiro variant below (a visible kitty window via `opencode-worker.sh`)
remains for the rare one-off the user wants visually navigable in the window manager;
it is not what the autonomous loop dispatches.

## 0. The default dispatch: a disposable sandbox, no enwiro at all (kanban ruling, 2026-09-06)

`dispatch-worker.sh` is retired -- the plain worktree-plus-background-process model it
implemented is a strict subset of what the sandbox does, and every permission-wall
failure on that path needed a sandboxed rescue anyway. The one dispatch mechanism now:

    nohup python3 .claude/scripts/sandbox_dispatch.py <row-id> --brief <path-to-brief-file> &

That runs the FULL cycle unsupervised in a disposable, fully-permissive microsandbox
microVM: plan-decompose (search for a simplifying refactor before writing code),
build, its own real `just check` verify with retries against the actual failure
evidence, patch extraction, `git am -3` onto a fresh `~/.local/share/toylang-lanes/issue-<row-id>`
lane, then `land-lane.sh land` directly -- 15-40 minutes end to end, never waited on
inline. Sandbox concurrency is capped at 3 (kanban ruling, 2026-09-06 -- measured
`lane-history.jsonl` data showed the practical concurrency ceiling was 3, not the old
plain-lane cap of 8); count truly in-progress dispatches with
`.claude/scripts/sandbox_dispatch_status.py --count` (a live host process), never
`msb list` (its "running" status stays true for a sandbox kept alive for anomaly
debugging long after the dispatch that owned it has already exited) or board.yaml's
`status: delegated` (never flipped back on an escalated row). Orphaned kept-for-debugging
sandboxes are reclaimed automatically every tick, no action needed.

No env, no workspace, no focus dance, no permission-wall boilerplate to teach around --
the sandbox is fully permissive by construction, which is the whole reason
`dispatch-worker.sh`'s `KNOWN DENIALS` accumulation is gone rather than extended
further. Unresolved runs (retry cap reached, a genuine `git am -3` conflict, an
extraction anomaly) route to the maintainer's mailbox automatically via
`compose_escalation()` (`docs/.grill/<row-id>-sandbox-blocker.round.yaml`) -- read and
act on these like any other wizard round, never by blindly redispatching while one is
still open.

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

Default: `sandbox_dispatch.py` (section 0). It picks its own models (`--model` for the
build turn, `--plan-model` for plan-decompose, `--critic-model` for the devil's-advocate
review; the board's `model:` field is dormant for builds) and lands directly on success
-- no separate "wrapper fires a drive tick" step, `sandbox_dispatch.py` calls
`land-lane.sh land` itself before its process exits.

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

### The brief: a plain task description, no boilerplate to wrap (2026-09-06)

`sandbox_dispatch.py` has no permission-wall boilerplate to teach around (the sandbox is
fully permissive by construction), so there is no `KNOWN DENIALS` list and no
`ESCALATION.md`-in-the-worktree convention to wrap the brief in either -- the brief file
passed to `--brief` is just the task description, plain text, written directly:

- pointers to the in-repo source of truth (files, the ruling issue, existing patterns
  to read first);
- for a research task: exactly where to write findings (e.g. `plans/<name>.md`) and
  that it should be committed;
- any extra done-gates beyond `just check` passing.

Two to eight sentences is normal; `sandbox_dispatch.py` itself supplies the
plan-decompose and build prompts around it (`PLAN_PROMPT_TEMPLATE`,
`BUILD_AFTER_DECOMPOSE` in the script). AGENTS.md is read natively by the opencode
session inside the sandbox, same as before.

A blocker the dispatch itself cannot resolve reaches the maintainer automatically --
`compose_escalation()` writes `docs/.grill/<row-id>-sandbox-blocker.round.yaml` and the
sandbox's own exit is the trigger, no `ESCALATION.md`-in-the-worktree convention needed.

### Research dispatches (2026-09-06): diagnosis is worker work too

A deep dive -- why a backend misbehaves, why a lane died mid-task, what an odd test
failure means -- is DELEGATED, never done by the coordinator in its own session:
coordinator time is the expensive tier now, and reading a codebase is exactly what a
cheap worker does well. Same `sandbox_dispatch.py`, same brief shape as any other
dispatch -- there is no separate raw-vs-wrapped mode to choose, since there is no build
boilerplate being wrapped in the first place. State the question plainly and where to
write the answer:

> Board row `<row-id>`: this is a RESEARCH task, no compiler code changes expected.
> [the precise question, with every symptom already known -- failing command, error
> text, suspect files]. Investigate freely (read code, run `just check`, reproduce).
> Write findings to `plans/<name>.md` and commit it.

`sandbox_dispatch.py` runs its normal cycle against this brief (plan-decompose still
fires, usually converges to "trivial" fast for a pure research task) and lands the
committed findings file exactly like a code change -- `--max-plan-rounds 0` skips
plan-decompose entirely for a brief that is already this precise. `--model`/`--plan-model`
can lift a hard question to a stronger model per-dispatch.

## 2b. Update the board

Set the row's `status: delegated`. The tick scripts resolve the worktree from the
issue number (`~/.local/share/toylang-lanes/issue-<N>` first, the legacy enwiro base as
fallback); the `lane:` field is legacy and set on no new row. A delegation without a
board row means the task skipped planning -- add the row.

## 3. Steering a running sandbox dispatch

There is no SendMessage into a sandboxed dispatch, and no session-resume primitive
either (unlike the retired `dispatch-worker.sh` path, sandbox dispatch is stateless
per attempt by design -- every fresh dispatch branches clean from `origin/main`, never
continuing a prior attempt's state). A run in progress cannot be redirected: let it
finish (green and landed, or escalated to `docs/.grill/`) rather than killing it
mid-flight. Feedback on a FAILED build turn is already automatic within one dispatch
-- `run_build_cycle()` feeds the real `just check` failure back to the same opencode
session on retry (`--continue`), up to `--retry-cap`. Once a dispatch has fully exited
(landed, or escalated), the only way to correct its course is a fresh dispatch with a
sharper brief -- there is no log-and-resume step, since the escalation round or the
landed diff already carries the evidence needed to write one.

## Cleanup

A landed `toylang-lanes` worktree is removed by the coordinator at landing
(`git worktree remove`, from the land skill) -- the branch and commits live in the main
repo's .git, so nothing is lost and disk stays flat. Enwiro envs stay the user's to
remove (`enw rm`), never with unmerged work without explicit instruction.
