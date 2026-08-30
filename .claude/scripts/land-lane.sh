#!/usr/bin/env bash
# Size-driven landing pipeline (maintainer design, 2026-08-30, superseding the
# single-wip two-stage flow the same evening): merges never block work, and the
# SIZE of a branch tells you its role.
#
#   land-lane.sh fold <merge-msg-file> <issue-number>...
#       Fold finished lane(s) into an accumulator branch (`to-merge-<epoch>`).
#       The first lane SEEDS a new accumulator when none exists -- literally the
#       lane's own commits under the to-merge name, no merge commit -- and later
#       lanes fold in with --no-ff. Target: the LARGEST accumulator not being
#       promoted (smallest-into-largest, per the ruling). Gate: `just check`.
#       If the fold pushes the accumulator past SIZE_LIMIT changed lines
#       (insertions + deletions vs main), promotion fires automatically,
#       DETACHED -- folding never waits on the full suite.
#
#   land-lane.sh promote <to-merge-branch>
#       Merge one accumulator into main behind the FULL `just test`, then push.
#       Mechanical by design (judgment happened at fold time, in the diff read),
#       so it is safe to run detached: the gate runs in a throwaway worktree and
#       main is only touched AFTER green -- a red gate leaves main untouched,
#       drops a promote-failed marker, and fires a tick to route the repair.
#
# Staleness is the tick's trigger, not this script's: an accumulator untouched
# for 30+ minutes is promoted as-is (drive-tick.sh names it).
set -uo pipefail
REPO=/home/kantord/repos/toylang
LANES="$HOME/.local/share/toylang-lanes"
LOG_DIR="$HOME/.cache/toylang-drive"
SIZE_LIMIT=600   # changed lines (insertions+deletions); typical lanes 50-700, measured 2026-08-30
MODE="${1:?usage: land-lane.sh fold <merge-msg-file> <issues...> | promote <branch>}"
shift
mkdir -p "$LOG_DIR"

worker_free() { # $1: dir that must have no live worker
  for p in $(pgrep -x opencode 2>/dev/null; pgrep -x claude 2>/dev/null); do
    case "$(readlink /proc/$p/cwd 2>/dev/null)" in "$1"*) return 1 ;; esac
  done
  return 0
}

changed_lines() { # $1: branch; insertions+deletions vs the merge base with main
  git -C "$REPO" diff --shortstat "main...$1" 2>/dev/null \
    | grep -oE '[0-9]+ insertion|[0-9]+ deletion' | grep -oE '[0-9]+' \
    | paste -sd+ | bc 2>/dev/null || echo 0
}

case "$MODE" in
fold)
  set -e
  MSG_FILE="${1:?merge message file required}"; shift
  [ $# -ge 1 ] || { echo "no issues given" >&2; exit 2; }
  for n in "$@"; do
    d="$LANES/issue-$n"
    [ -d "$d" ] || { echo "refusing: no worktree $d" >&2; exit 2; }
    [ "$(git -C "$d" status --porcelain | wc -l)" -eq 0 ] || { echo "refusing: issue-$n dirty" >&2; exit 2; }
    worker_free "$d" || { echo "refusing: live worker in issue-$n" >&2; exit 2; }
  done

  # Target: the largest accumulator that is not mid-promotion and not full.
  ACC=""; ACC_SIZE=-1
  for b in $(git -C "$REPO" for-each-ref --format='%(refname:short)' 'refs/heads/to-merge-*'); do
    [ -f "$LOG_DIR/promoting-$b" ] && continue
    s=$(changed_lines "$b")
    [ "$s" -ge "$SIZE_LIMIT" ] && continue
    [ "$s" -gt "$ACC_SIZE" ] && { ACC="$b"; ACC_SIZE=$s; }
  done

  REST=("$@")
  if [ -z "$ACC" ]; then
    # Seed: the first lane's commits become the accumulator, aliased -- the
    # branch it worked in under the to-merge name (maintainer's shape).
    n="${REST[0]}"; REST=("${REST[@]:1}")
    ACC="to-merge-$(date +%s)"
    git -C "$REPO" branch "$ACC" "$(git -C "$REPO" rev-parse "issue-$n")"
    git -C "$REPO" worktree remove "$LANES/issue-$n"
    git -C "$REPO" branch -d "issue-$n" 2>/dev/null || true
    echo "[land-lane] seeded $ACC from issue-$n"
  fi
  ACC_DIR="$LANES/.acc-$ACC"
  [ -d "$ACC_DIR" ] || git -C "$REPO" worktree add "$ACC_DIR" "$ACC" -q
  [ -f "$ACC_DIR/.git/MERGE_HEAD" ] 2>/dev/null && { echo "refusing: $ACC mid-merge" >&2; exit 2; }
  [ "$(git -C "$ACC_DIR" status --porcelain | wc -l)" -eq 0 ] || { echo "refusing: $ACC dirty" >&2; exit 2; }
  for n in "${REST[@]+"${REST[@]}"}"; do
    git -C "$ACC_DIR" merge "issue-$n" --no-ff -F "$MSG_FILE"
  done
  (cd "$ACC_DIR" && just check)
  git -C "$ACC_DIR" push -u origin "$ACC" 2>/dev/null || true
  for n in "${REST[@]+"${REST[@]}"}"; do
    git -C "$REPO" worktree remove "$LANES/issue-$n"
    git -C "$REPO" branch -d "issue-$n" 2>/dev/null || true
  done
  SIZE=$(changed_lines "$ACC")
  echo "[land-lane] $ACC took: $* (now ${SIZE} changed lines, limit $SIZE_LIMIT)"
  if [ "$SIZE" -ge "$SIZE_LIMIT" ]; then
    echo "[land-lane] $ACC is FULL -- promoting detached"
    (nohup "$REPO/.claude/scripts/land-lane.sh" promote "$ACC" \
      >>"$LOG_DIR/promote.log" 2>&1 &)
  fi
  ;;
promote)
  B="${1:?to-merge branch required}"
  git -C "$REPO" rev-parse -q --verify "$B" >/dev/null || { echo "no such branch $B" >&2; exit 2; }
  exec 8>"/tmp/toylang-promote.lock"
  flock -w 1200 8 || { echo "[promote] another promotion held the lock 20+ min -- gave up" >&2; exit 1; }
  touch "$LOG_DIR/promoting-$B"
  trap 'rm -f "$LOG_DIR/promoting-$B"' EXIT
  TIP=$(git -C "$REPO" rev-parse "$B")
  TMP="promote-$(date +%s)"
  PDIR="$LANES/.promote"
  git -C "$REPO" worktree remove --force "$PDIR" 2>/dev/null || true
  git -C "$REPO" worktree add -b "$TMP" "$PDIR" main -q
  cleanup_tmp() {
    git -C "$REPO" worktree remove --force "$PDIR" 2>/dev/null || true
    git -C "$REPO" branch -D "$TMP" 2>/dev/null || true
  }
  # Gate in the throwaway worktree: main stays untouched until green.
  if ! { git -C "$PDIR" merge "$TIP" --no-ff \
           -m "Land $B: promote $(git -C "$REPO" rev-list --count main..$TIP) commits ($(changed_lines "$B") changed lines)" \
         && (cd "$PDIR" && just test); }; then
    cleanup_tmp
    echo "[promote] RED: $B failed the full gate; main untouched" | tee "$LOG_DIR/promote-failed-$B"
    (nohup "$REPO/.claude/scripts/drive-tick.sh" >>"$LOG_DIR/event-ticks.log" 2>&1 &)
    exit 1
  fi
  # Green: fold the tested result into main. Retry around a busy tick's commit.
  ok=0
  for _ in $(seq 24); do
    if [ "$(git -C "$REPO" status --porcelain | wc -l)" -eq 0 ] \
       && [ ! -f "$REPO/.git/MERGE_HEAD" ] \
       && git -C "$REPO" merge "$TMP" --no-edit; then ok=1; break; fi
    git -C "$REPO" merge --abort 2>/dev/null || true
    sleep 5
  done
  if [ "$ok" -ne 1 ]; then
    cleanup_tmp
    echo "[promote] could not merge into main (checkout stayed busy/dirty)" | tee "$LOG_DIR/promote-failed-$B"
    (nohup "$REPO/.claude/scripts/drive-tick.sh" >>"$LOG_DIR/event-ticks.log" 2>&1 &)
    exit 1
  fi
  git -C "$REPO" push
  cleanup_tmp
  # -d, never -D: a lane folded in mid-promotion leaves the branch unmerged,
  # and then it must SURVIVE to carry those commits to the next promotion.
  if git -C "$REPO" branch -d "$B" 2>/dev/null; then
    git -C "$REPO" worktree remove --force "$LANES/.acc-$B" 2>/dev/null || true
    git -C "$REPO" push origin --delete "$B" 2>/dev/null || true
  else
    echo "[promote] $B grew during promotion -- kept with its unpromoted commits"
  fi
  rm -f "$LOG_DIR/promote-failed-$B"
  echo "[promote] $B -> main: $(git -C "$REPO" log --merges --format=%s -1) (pushed)"
  ;;
wip)
  echo "the wip flow is retired (same-day): use fold/promote" >&2; exit 2 ;;
*)
  echo "unknown mode: $MODE" >&2; exit 2 ;;
esac
