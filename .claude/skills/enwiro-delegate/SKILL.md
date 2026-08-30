---
type: Playbook
name: enwiro-delegate
description: Delegate a task to a fresh interactive Claude Code session in its own enwiro environment (git worktree) and workspace, visually navigable for the user. Use when the user wants implementation handed to a separate session/worktree/workspace, says "delegate this", "spawn a session for this", or names enwiro delegation.
---

# Delegate a task to a new enwiro environment

Launch a fresh, *interactive* Claude Code session in its own enwiro environment so the user
can switch to it visually (window-manager workspace), take it over, or leave it running.
Never use `claude -p` for this: headless output is not navigable and cannot be taken over.

## 0. Board rows go to the worker pool first (gh:124)

Board dispatches reuse a small pool of persistent lanes -- enwiro envs named
`toylang@lane-1` .. `toylang@lane-5`, cooked lazily with `enw prep` -- instead of a fresh
env per issue. A cold session spends roughly 30-50k tokens booting (system prompt,
CLAUDE.md, AGENTS.md, repo orientation; a bare session in an empty directory measured
29k) before doing any work; a reused session has already paid that. Even a recycled lane
keeps its worktree, so the cargo `target/` cache and the disk it occupies stop
multiplying per task. One-off delegations the user asks for by hand still use the
fresh-env flow below; design rationale and open questions live in
[plans/worker-pool.md](../../../plans/worker-pool.md).

A lane is **free** when its worktree is clean and its last task's board row has been
archived. A free lane with no session yet is **cold**: dispatch into it with the normal
launch below, just inside the lane env. A **warm** lane gets an eligibility check:

    wt=$(enw prep 'toylang@lane-1')
    python3 .claude/scripts/lane-context.py "$wt"
    # sid=<id> model=<model> context=<tokens|unknown> source=<transcript|telemetry|none>

Reuse the session only when ALL of these hold; otherwise recycle (fresh session, same
worktree):

- context is a number under MAX_LANE_CONTEXT (90000, the same ceiling drive-tick.sh
  enforces on the coordinator session; provisional until lanes.csv holds worker rows).
- if context is `unknown` (interactive sessions on the newer harness expose no
  transcript, and telemetry only lands when a session ends), the backstop is the task
  count: fewer than 3 dispatches logged for this sid in
  `~/.cache/toylang-drive/dispatches.csv`.
- the session's model matches the row's tier exactly. A haiku row must not run in a
  fable session's lane or the reverse: the tier judgment on the board row and the
  accumulated context both leak across otherwise.
- the previous task landed without a stall intervention. A lane that needed rescuing
  is confused; recycle it.

Recycling requires the old worker process to be gone: never launch a second session
into a worktree with a live one (the known two-sessions-one-worktree race). A live lane
that fails the reuse rules is not dispatched to at all; pick another lane or cook the
next one.

### Prepare the worktree (the coordinator does this before any pool dispatch)

    git -C "$wt" fetch origin
    git -C "$wt" switch -C issue-<N> origin/main

Only on a clean tree -- a dirty pool lane is not free, it is unlanded work. If branch
`issue-<N>` is already checked out in another worktree this fails; that task already has
an env, and a continuation dispatch there is what's wanted, not a pool lane.

### Dispatch into the lane

Log it first (this also feeds the per-task cost numbers the ceiling will be tuned on):

    python3 .claude/scripts/lane-context.py "$wt" --log-dispatch gh:<N>

Then reach the session, one of three ways:

- **Worker process alive** (pgrep claude, cwd under the resolved worktree): SendMessage
  to it -- ListAgents shows it named after the worktree basename (`lane-1-3c0...`).
  Verified 2026-08-30: an idle interactive session processes the message as a new turn.
- **Process gone, session reusable**: relaunch with the same kitty dance as step 2
  below, but `claude --resume <sid> '<brief>'` in place of the bare prompt. Verified
  2026-08-30: a resumed session retains its prior context.
- **Recycling, or a cold lane**: the normal launch of step 2, inside the lane env.

A reused session needs in its brief what a fresh boot gets for free (the issue-branch
rule only fires at session start), plus the context-hygiene line:

> Next task for this lane: gh issue #<N>. The worktree is on a fresh branch issue-<N>
> cut from origin/main. Your previous task is landed history: do not carry its
> decisions or context into this one; re-derive everything from the issue and the
> repo. Run `gh issue view <N>` before touching anything.

followed by the same closing instruction every kickoff prompt gets (step 2). Provenance
stays per-commit and per-branch, so one session spanning tasks changes nothing there;
the hygiene line exists because residual context is the one channel through which task
A could silently author task B.

Bookkeeping: set `lane: lane-<N>` on the board row next to `status: delegated` (the
tick scripts resolve the worktree through it), and keep the env navigable despite its
generic name:

    enw goal set --env 'toylang@lane-1' 'gh:<N> <short title>'
    enw mark active --env 'toylang@lane-1'

## 1. Pick the environment name (this decides how the new session gets briefed)

For a board dispatch the name is decided already: the pool lane (section 0). The choice
below is for one-off delegations outside the pool.

- `repo#<issue>` (github recipe) -- **preferred**. The env's branch name carries the issue
  number, so the global issue-linked-branches rule makes the new session fetch its own brief
  with `gh issue view` before doing anything. If no issue exists yet, create one first
  (`gh issue create`) pointing at the in-repo plan/spec; push any local commits the issue
  references BEFORE filing. Public repo issues must be sanitized per the data-privacy rules.
- `repo@<branch-name>` (git recipe) -- for work with no issue. The new session gets its brief
  only from the kickoff prompt and in-repo docs, so name the source-of-truth file explicitly.

## 1b. Push first -- worktrees branch from origin, not local main

The github cookbook cuts the delegation worktree from **origin's** default branch. If local
main is ahead and unpushed, the session builds against a stale base and its branch merges
back with semantic drift the suite only catches on main (this reverted the anyhow work once
before being caught). Before dispatching: push local main yourself (standing authorization,
2026-08-29 -- ordinary pushes of main only, never force). Never dispatch onto a stale origin.

## 2. Activate, launch kitty with claude inside, then switch the user back

```sh
prev=$(i3-msg -t get_workspaces | jq -r '.[] | select(.focused).name')
enw activate 'toylang#12'
enw wrap kitty 'toylang#12' -- --detach claude --model sonnet 'Implement step 1 of plans/enums.md. Read AGENTS.md first.'
sleep 4   # let the kitty window map on the env workspace, and verify the session is live
i3-msg "workspace \"$prev\"" >/dev/null
```

Regular build tasks run on `--model sonnet`; that is the default. Omit the flag (falling back
to the account default, Fable) only when the board row says `model: fable` -- reserved for
design-heavy or cross-cutting work where the coordinator judges the bigger model earns its
cost.

- `enw activate` cooks the env (worktree + branch) and binds/switches to its workspace,
  which yanks the user's focus. Capture the focused workspace BEFORE activating and switch
  back as the last step, so the delegation is barely noticeable: the sleep matters, because
  the kitty window must map while the env workspace is focused, and it doubles as the moment
  to verify the claude process is live (pgrep with cwd under the env worktree).
- `enw wrap <cmd> <env> -- <args>` runs the command with the env's directory as cwd and
  `ENWIRO_ENV` exported (verified), so the child session's scratchpad protocol resolves to the
  right place. `kitty --detach` forks, so the launching shell returns immediately.
- The kickoff prompt is one or two sentences naming the task's in-repo source of truth. For a
  `repo#issue` env the issue rule does the heavy lifting; do not duplicate the issue body.
- Every kickoff prompt ends with this instruction, verbatim: "When the work is finished,
  COMMIT it. Never end your turn to present work for approval, and never wait on a human for
  a decision: if something needs deciding that your brief does not settle, file a GitHub
  issue for the coordinator with the alternatives and costs, take the most conservative
  continuation, and keep going. Nobody reads this chat; the coordinator reviews after you
  commit." Sessions that finish and wait politely are the most common stall (it has happened
  twice), and from outside, a turn-end is indistinguishable from a crash. The maintainer
  interacts only with the coordinator, never with delegated sessions.

## 2b. Update the board

If `plans/board.yaml` exists and has a row for this task, set its `status: delegated` (see
the `drive` skill). A delegation without a board row means the task skipped planning -- add
the row.

## 3. Hand over and keep in touch

- Tell the user the env name; they switch to it with their WM or `enw activate '<name>'`,
  and track running work with `enw kanban`. Status changes: `enw mark`.
- The new session is a sibling, not a subagent: find it with ListAgents and use SendMessage
  to nudge or query it. Do not duplicate its work in your own session.
- If the user wants file-based steering there, tell them to run `/sp-start` in that session.

## Cleanup

After the branch is reviewed and merged (the user's call), `enw rm '<name>'` removes the
environment. Never remove an env with unmerged work without explicit instruction.
