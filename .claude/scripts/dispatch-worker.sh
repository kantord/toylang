#!/usr/bin/env bash
# Enwiro-free delegation (maintainer simplification, 2026-08-30): a lane is just a
# git worktree under ~/.local/share/toylang-lanes plus a background opencode worker.
# No env, no workspace, no focus dance -- observability is the live log this prints.
#
# Usage: dispatch-worker.sh <issue-number> '<kickoff brief>'
# Continuation dispatches reuse the existing worktree (never with a live worker).
set -euo pipefail
REPO=/home/kantord/repos/toylang
LANES="$HOME/.local/share/toylang-lanes"
LOG_DIR="$HOME/.cache/toylang-drive/opencode"
N="${1:?usage: dispatch-worker.sh <issue-number> '<brief>'}"
BRIEF="${2:?usage: dispatch-worker.sh <issue-number> '<brief>'}"
d="$LANES/issue-$N"
mkdir -p "$LANES" "$LOG_DIR"

for p in $(pgrep -x opencode 2>/dev/null; pgrep -x claude 2>/dev/null); do
  case "$(readlink /proc/$p/cwd 2>/dev/null)" in "$d"*)
    echo "refusing: live worker (pid $p) owns $d" >&2; exit 1 ;;
  esac
done

git -C "$REPO" fetch -q origin
# Lanes cut from wip when the two-stage pipeline is running (landed-but-unpromoted
# work must be buildable-upon), from main otherwise.
BASE=origin/main
git -C "$REPO" rev-parse -q --verify origin/wip >/dev/null 2>&1 && BASE=origin/wip
if [ -d "$d" ]; then
  echo "[dispatch] continuing in existing lane $d"
else
  # -B would steal a branch checked out elsewhere; add fails loudly instead,
  # which is the guard we want (that task already has a worktree).
  git -C "$REPO" worktree add -b "issue-$N" "$d" "$BASE" -q 2>/dev/null \
    || git -C "$REPO" worktree add "$d" "issue-$N" -q
fi

LIVE="$LOG_DIR/issue-$N.live.log"
(cd "$d" && nohup "$REPO/.claude/scripts/opencode-worker.sh" "$BRIEF" \
  >>"$LIVE" 2>&1 &)
echo "[dispatch] issue-$N worker launched; watch: tail -f $LIVE"
