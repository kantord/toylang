#!/usr/bin/env bash
# Two-stage landing pipeline (maintainer design, 2026-08-30): merges never block work.
#
#   land-lane.sh wip <merge-msg-file> <issue-number>...
#       Merge finished lane(s) into the running `wip` branch (its own worktree at
#       $WIP_DIR, created from main on first use), gate with `just check` (the fast
#       loop), push origin/wip, remove the landed lane worktrees.
#
#   land-lane.sh promote <merge-msg-file>
#       Merge wip into main from the repo root, gate with the FULL `just test`,
#       push main. The one heavy suite pays for every lane batched since the last
#       promotion (typical lanes are 50-700 lines; 3-5 per batch, measured 2026-08-30).
#
# Judgment stays upstream (the coordinator's diff read and verdict); any failure here
# stops loudly before the push. A red promotion FREEZES wip: repair it before merging
# more lanes into it (the skill carries that rule).
set -euo pipefail
REPO=/home/kantord/repos/toylang
LANES="$HOME/.local/share/toylang-lanes"
WIP_DIR="$LANES/.wip"
MODE="${1:?usage: land-lane.sh wip|promote <merge-msg-file> [issues...]}"
MSG_FILE="${2:?merge message file required}"
shift 2

worker_free() { # $1: dir that must have no live worker
  for p in $(pgrep -x opencode 2>/dev/null; pgrep -x claude 2>/dev/null); do
    case "$(readlink /proc/$p/cwd 2>/dev/null)" in "$1"*) return 1 ;; esac
  done
  return 0
}

case "$MODE" in
wip)
  [ $# -ge 1 ] || { echo "no issues given" >&2; exit 2; }
  if [ ! -d "$WIP_DIR" ]; then
    git -C "$REPO" branch -f wip main 2>/dev/null || git -C "$REPO" branch wip main
    git -C "$REPO" worktree add "$WIP_DIR" wip
  fi
  [ -f "$WIP_DIR/.git/MERGE_HEAD" ] 2>/dev/null && { echo "refusing: wip mid-merge" >&2; exit 2; }
  [ "$(git -C "$WIP_DIR" status --porcelain | wc -l)" -eq 0 ] || { echo "refusing: wip dirty" >&2; exit 2; }
  for n in "$@"; do
    d="$LANES/issue-$n"
    [ -d "$d" ] || { echo "refusing: no worktree $d" >&2; exit 2; }
    [ "$(git -C "$d" status --porcelain | wc -l)" -eq 0 ] || { echo "refusing: issue-$n dirty" >&2; exit 2; }
    worker_free "$d" || { echo "refusing: live worker in issue-$n" >&2; exit 2; }
  done
  for n in "$@"; do
    git -C "$WIP_DIR" merge "issue-$n" --no-ff -F "$MSG_FILE"
  done
  (cd "$WIP_DIR" && just check)
  git -C "$WIP_DIR" push origin wip
  for n in "$@"; do
    git -C "$REPO" worktree remove "$LANES/issue-$n"
  done
  echo "[land-lane] wip took: $* (gated by just check; promotion runs the full suite)"
  ;;
promote)
  cd "$REPO"
  [ -f .git/MERGE_HEAD ] && { echo "refusing: merge in progress on main" >&2; exit 2; }
  git status --porcelain | grep -q . && { echo "refusing: main checkout dirty" >&2; exit 2; }
  [ "$(git branch --show-current)" = main ] || { echo "refusing: not on main" >&2; exit 2; }
  [ "$(git rev-list --count main..wip 2>/dev/null || echo 0)" -gt 0 ] || { echo "nothing to promote" >&2; exit 0; }
  git merge wip --no-ff -F "$MSG_FILE"
  just test
  git push
  echo "[land-lane] promoted wip -> main ($(git log --merges --format=%s -1))"
  ;;
*)
  echo "unknown mode: $MODE" >&2; exit 2 ;;
esac
