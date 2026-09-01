#!/usr/bin/env bash
# Serial landing queue (maintainer redesign, 2026-09-01, superseding the
# size-driven accumulator pipeline of 2026-08-30 -- fold/promote and the
# to-merge-* branches are retired):
#
#   land-lane.sh land <issue-number>...
#
# One lane at a time, straight onto main, behind the FULL `just test` in a
# throwaway worktree -- main is only touched after green. Lands serialize on a
# flock. A merge conflict or a red gate never blocks the queue: the script
# writes the evidence to LAND-FAILURE.txt in the lane worktree, re-dispatches
# the lane's own worker with a templated repair brief (cap: 2 automatic
# retries, tracked in $LOG_DIR/land-retries-issue-N), then moves to the next
# candidate. The third failure leaves $LOG_DIR/land-failed-issue-N for the
# tick to escalate into a maintainer round.
#
# Deterministic by design (maintainer ruling, 2026-09-01): no model reads the
# diff before landing -- `just test` is the whole pre-merge gate, and review
# happens post-land, asynchronously, in the tick. The merge message is
# generated from the lane's own commit subjects.
set -uo pipefail
REPO=/home/kantord/repos/toylang
LANES="$HOME/.local/share/toylang-lanes"
LOG_DIR="$HOME/.cache/toylang-drive"
SCRIPTS="$REPO/.claude/scripts"
RETRY_CAP=2
mkdir -p "$LOG_DIR"
cd "$REPO"  # never run with cwd inside a worktree this script may remove

MODE="${1:?usage: land-lane.sh land <issue-number>...}"
shift

worker_free() { # $1: dir that must have no live worker
  for p in $(pgrep -x opencode 2>/dev/null; pgrep -x claude 2>/dev/null); do
    case "$(readlink /proc/$p/cwd 2>/dev/null)" in "$1"*) return 1 ;; esac
  done
  return 0
}

# 8>&- everywhere a child is spawned: fd 8 carries the land flock, and any
# child inheriting it keeps the whole queue locked for its own lifetime -- a
# retriggered WORKER held the lock through its 20-minute run and three lands
# queued behind it (2026-09-01, the same disease as the tick's fd-9 leak).
fire_tick() {
  (nohup "$SCRIPTS/drive-tick.sh" >>"$LOG_DIR/event-ticks.log" 2>&1 &) 8>&-
}

# Red gate or conflict: write evidence into the lane (untracked scratch is
# sanctioned and cleaned on the next land attempt), then either re-dispatch
# the lane's worker with a templated brief or, past the cap, leave the marker.
retrigger() { # $1: issue number  $2: short failure kind  $3: evidence file
  local n=$1 kind=$2 evidence=$3 d="$LANES/issue-$1" count
  count=$(( $(cat "$LOG_DIR/land-retries-issue-$n" 2>/dev/null || echo 0) + 1 ))
  echo "$count" >"$LOG_DIR/land-retries-issue-$n"
  cp "$evidence" "$d/LAND-FAILURE.txt" 2>/dev/null || true
  if [ "$count" -gt "$RETRY_CAP" ]; then
    echo "landing issue-$n: $kind, attempt $count -- retry cap reached" \
      >"$LOG_DIR/land-failed-issue-$n"
    echo "[land] issue-$n: $kind on attempt $count -- CAP REACHED, left for escalation"
    return
  fi
  echo "[land] issue-$n: $kind on attempt $count -- re-dispatching the lane worker"
  "$SCRIPTS/dispatch-worker.sh" "$n" "A previous worker completed this task, but landing the branch on main FAILED: $kind (landing attempt $count of $((RETRY_CAP + 1))). Read LAND-FAILURE.txt at the worktree root for the exact evidence before touching anything. Your job now is ONLY to make this branch land: merge origin/main into this branch, resolve any conflicts in favor of intent (both sides' tests must still pass), fix whatever LAND-FAILURE.txt shows failing, and re-run the gate. Do not start new feature work." 8>&- \
    || echo "[land] issue-$n: re-dispatch refused (see above)"
}

case "$MODE" in
land)
  [ $# -ge 1 ] || { echo "no issues given" >&2; exit 2; }
  # One land at a time, machine-wide. Bounded wait with an explicit give-up
  # (house pattern): the periodic tick is the backstop that re-fires a land
  # that gave up here.
  exec 8>"/tmp/toylang-land.lock"
  flock -w 1800 8 || { echo "[land] queue lock held 30+ min -- gave up (tick will retry)" >&2; fire_tick; exit 1; }
  ANY_GREEN=0
  for n in "$@"; do
    d="$LANES/issue-$n"
    B="issue-$n"
    [ -d "$d" ] || { echo "[land] skip issue-$n: no worktree $d"; continue; }
    worker_free "$d" || { echo "[land] skip issue-$n: live worker"; continue; }
    if [ -n "$(git -C "$d" status --porcelain | grep -v '^??')" ]; then
      echo "[land] skip issue-$n: uncommitted tracked changes (not done)"; continue
    fi
    # Untracked leftovers are sanctioned scratch (workers cannot rm); drop them.
    if [ "$(git -C "$d" status --porcelain | grep -c '^??')" -gt 0 ]; then
      git -C "$d" clean -fdq
    fi
    if [ "$(git -C "$REPO" rev-list --count "main..$B")" -eq 0 ]; then
      echo "[land] skip issue-$n: nothing ahead of main"; continue
    fi

    # Deterministic merge message from the lane's own commits.
    MSG_FILE="$LOG_DIR/land-msg-issue-$n.txt"
    {
      echo "Land issue-$n: $(git log -1 --format=%s "$B")"
      echo
      git log --reverse --format='- %s' "main..$B"
    } >"$MSG_FILE"

    # Gate in a throwaway worktree: main stays untouched until green.
    GATE_LOG="$LOG_DIR/land-gate-issue-$n.log"
    TMP="land-tmp-$n"
    PDIR="$LANES/.land"
    git worktree remove --force "$PDIR" 2>/dev/null || true
    git branch -D "$TMP" 2>/dev/null || true
    git worktree add -b "$TMP" "$PDIR" main -q
    cleanup_tmp() {
      git worktree remove --force "$PDIR" 2>/dev/null || true
      git branch -D "$TMP" 2>/dev/null || true
    }
    if ! git -C "$PDIR" merge "$B" --no-ff -F "$MSG_FILE" >"$GATE_LOG" 2>&1; then
      { echo "MERGE CONFLICT merging origin/main + this branch:";
        git -C "$PDIR" diff --name-only --diff-filter=U; } >>"$GATE_LOG" 2>&1
      git -C "$PDIR" merge --abort 2>/dev/null || true
      cleanup_tmp
      retrigger "$n" "merge conflict with main" "$GATE_LOG"
      continue
    fi
    if ! (cd "$PDIR" && just test) >>"$GATE_LOG" 2>&1; then
      tail -n 60 "$GATE_LOG" >"$GATE_LOG.tail" && mv "$GATE_LOG.tail" "$GATE_LOG"
      cleanup_tmp
      retrigger "$n" "the full test suite went red" "$GATE_LOG"
      continue
    fi

    # Green: land the tested result. Bounded retry around a busy tick's
    # board commit in the main checkout; lane branches never touch plans/,
    # so a moved main cannot conflict here.
    ok=0
    for _ in $(seq 12); do
      if [ "$(git -C "$REPO" status --porcelain | wc -l)" -eq 0 ] \
         && [ ! -f "$REPO/.git/MERGE_HEAD" ] \
         && git -C "$REPO" merge "$TMP" -F "$MSG_FILE" >/dev/null 2>&1; then ok=1; break; fi
      git -C "$REPO" merge --abort 2>/dev/null || true
      sleep 5
    done
    cleanup_tmp
    if [ "$ok" -ne 1 ]; then
      # The lane is fine -- the checkout stayed busy. No retry burned, no
      # re-dispatch; the marker routes the tick to just re-run the land.
      echo "landing issue-$n: main checkout stayed busy/dirty -- re-run land-lane.sh land $n" \
        >"$LOG_DIR/land-failed-issue-$n"
      echo "[land] issue-$n: main checkout busy -- deferred (marker left for the tick)"
      continue
    fi
    git -C "$REPO" push
    git worktree remove --force "$d" 2>/dev/null || git worktree remove "$d"
    git branch -d "$B" 2>/dev/null || true
    rm -f "$LOG_DIR/land-retries-issue-$n" "$LOG_DIR/land-failed-issue-$n" \
          "$LOG_DIR/escalated-issue-$n" "$MSG_FILE" "$GATE_LOG"
    ANY_GREEN=1
    echo "[land] issue-$n -> main: $(git -C "$REPO" log --merges --format=%s -1) (pushed)"
  done
  # One tick per invocation: board-archive moves and post-land review on
  # green, escalation routing on failure, rebrief logic when nothing landed.
  fire_tick
  [ "$ANY_GREEN" -eq 1 ] || exit 1
  ;;
fold | promote | wip)
  echo "the accumulator pipeline is retired (2026-09-01): use land-lane.sh land <issue>" >&2
  exit 2 ;;
*)
  echo "unknown mode: $MODE" >&2; exit 2 ;;
esac
