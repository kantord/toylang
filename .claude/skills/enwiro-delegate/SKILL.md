---
type: Playbook
name: enwiro-delegate
description: Delegate a task to a fresh interactive Claude Code session in its own enwiro environment (git worktree) and workspace, visually navigable for the user. Use when the user wants implementation handed to a separate session/worktree/workspace, says "delegate this", "spawn a session for this", or names enwiro delegation.
---

# Delegate a task to a new enwiro environment

Launch a fresh, *interactive* Claude Code session in its own enwiro environment so the user
can switch to it visually (window-manager workspace), take it over, or leave it running.
Never use `claude -p` for this: headless output is not navigable and cannot be taken over.

## 1. Pick the environment name (this decides how the new session gets briefed)

- `repo#<issue>` (github recipe) -- **preferred**. The env's branch name carries the issue
  number, so the global issue-linked-branches rule makes the new session fetch its own brief
  with `gh issue view` before doing anything. If no issue exists yet, create one first
  (`gh issue create`) pointing at the in-repo plan/spec; push any local commits the issue
  references BEFORE filing. Public repo issues must be sanitized per the data-privacy rules.
- `repo@<branch-name>` (git recipe) -- for work with no issue. The new session gets its brief
  only from the kickoff prompt and in-repo docs, so name the source-of-truth file explicitly.

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
  COMMIT it. Never end your turn to present work for approval -- nobody reads this chat; the
  coordinator reviews after you commit." Sessions that finish and wait politely are the most
  common stall (it has happened twice), and from outside, a turn-end is indistinguishable
  from a crash.

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
