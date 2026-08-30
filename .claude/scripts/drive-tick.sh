#!/usr/bin/env bash
# Stateless drive tick. Fired from the user crontab, survives reboots and needs no
# long-lived coordinator session: each tick is a fresh `claude -p` that reconstructs all
# state from disk (board, worktrees, annotation stores) per the drive skill, running in
# auto permission mode -- the same classifier guardrail interactive sessions get.
# Model is chosen here: sonnet for routine monitoring/dispatch, fable when a lane looks
# landable, because landing is where review-finding judgment happens. `audit` as $1 runs
# the periodic audit prompt instead (always fable).
set -uo pipefail
REPO=/home/kantord/repos/toylang
WORKTREES=/home/kantord/.local/share/enwiro/worktrees/pr/toylang-1234138d
LOG_DIR="$HOME/.cache/toylang-drive"
mkdir -p "$LOG_DIR"

# Never two ticks at once: a landing tick can outlive several cron intervals.
exec 9>/tmp/toylang-drive-tick.lock
flock -n 9 || exit 0

export PATH="$HOME/.local/bin:$HOME/.local/share/pnpm:/usr/local/bin:/usr/bin:/bin"
cd "$REPO"

# The maintainer's mail UI depends on the dev server; revive it if a reboot ate it.
if ! curl -s -o /dev/null --max-time 3 http://localhost:5173/toylang/dev/; then
  (cd "$REPO/site" && nohup pnpm dev --port 5173 --strictPort \
    >>"$LOG_DIR/devserver.log" 2>&1 &)
fi

MODEL=sonnet
for wt in $(python3 -c "
import yaml
for r in yaml.safe_load(open('plans/board.yaml')):
    if r.get('status') == 'delegated' and str(r.get('issue', '')).startswith('gh:'):
        print('issue-' + r['issue'][3:])
"); do
  d="$WORKTREES/$wt"
  [ -d "$d" ] || continue
  ahead=$(git -C "$d" rev-list --count main..HEAD 2>/dev/null || echo 0)
  dirty=$(git -C "$d" status --porcelain 2>/dev/null | wc -l)
  if [ "$ahead" -gt 0 ] && [ "$dirty" -eq 0 ]; then
    # Quiet for 8 minutes (lane-watch's old threshold) means done, not pausing.
    recent=$(find "$d" -name .git -prune -o -name target -prune -o -type f \
      -newermt '-8 minutes' -print -quit 2>/dev/null)
    [ -z "$recent" ] && MODEL=fable
  fi
done

if [ "${1:-tick}" = "audit" ]; then
  MODEL=fable
  PROMPT='Periodic audit (drive skill, "The periodic audit" section) for toylang at /home/kantord/repos/toylang. You are a stateless coordinator: reconstruct everything from disk. Check: every open GitHub issue maps to a board row; every delegated row has a live or accounted-for lane; no worktree holds unmerged commits the board thinks landed; no falsely-stuck lanes. Fix what is mechanical, file issues for the rest. End quietly if clean.'
else
  PROMPT='Drive tick (drive skill, monitoring phase) for toylang at /home/kantord/repos/toylang. You are a STATELESS coordinator session with no history: reconstruct all state from disk -- plans/board.yaml, the worktrees, memory -- and trust disk over assumptions. Read board rows with status: delegated and check each lane worktree (commits vs main, dirty files, live worker via pgrep cwd). Poll BOTH annotation stores: docs/.annotations/inbox.json AND docs/.annotations/notes.json -- apply entries older than 5 minutes and clear them at capture; wizard submissions in docs/.grill/ process immediately (delete round files at capture). If a lane is finished (ahead of main, clean, 8+ minutes quiet or worker gone), run the land-delegated-work skill -- cascade if 3+ are ready. After landings, dispatch ready board rows into free lanes (cap 5, model per row: haiku/sonnet/fable). If nothing changed, end quietly without writing anything.'
fi

TS=$(date +%Y%m%d-%H%M%S)
claude -p --model "$MODEL" --permission-mode auto "$PROMPT" \
  >>"$LOG_DIR/$TS-${1:-tick}-$MODEL.log" 2>&1
