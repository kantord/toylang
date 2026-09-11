#!/usr/bin/env bash
# Serial landing queue (maintainer redesign, 2026-09-01, superseding the
# size-driven accumulator pipeline of 2026-08-30 -- fold/promote and the
# to-merge-* branches are retired):
#
#   land-lane.sh land <issue-number>...
#   land-lane.sh land-patch <row-id> <patch-file>
#
# One lane at a time, straight onto main, behind the FULL `just test` in a
# throwaway worktree -- main is only touched after green. Lands serialize on a
# flock. A merge conflict or a red gate never blocks the queue: the script
# re-dispatches via simple_dispatch.py with a templated repair brief carrying
# the evidence (cap: 2 automatic retries, tracked in
# $LOG_DIR/land-retries-issue-N), then moves to the next candidate. The third
# failure leaves $LOG_DIR/land-failed-issue-N for the tick to escalate into a
# maintainer round.
#
# `land-patch` (added 2026-09-11, simple_dispatch.py's only dispatch
# mechanism) is an adapter: simple_dispatch.py never creates a lane worktree,
# it produces a plain `git format-patch` file from a disposable clone.
# `land-patch` materializes the same worktree/branch `land` itself expects
# from that patch (`git am` onto a fresh branch off main), then runs the
# exact same landing logic -- gate, merge, push, retry-on-failure -- as
# `land` does for any other row.
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
# The gate and the regeneration path need cargo regardless of who fired us
# (worker trap, tick, interactive shell).
export PATH="$HOME/.cargo/bin:$PATH"
command -v sccache >/dev/null && export RUSTC_WRAPPER=sccache
# Files whose merge conflicts are RESOLVABLE BY REGENERATION (maintainer
# ruling, 2026-09-01): corpus.json appeared in every conflict of the queue's
# first night; burning a worker on computable content was the dominant retry
# cost. A conflict touching ONLY these is resolved mechanically below.
GENERATED='site/public/corpus.json tests/snapshots/backend_llvm__native_agrees_where_it_compiles.snap tests/snapshots/backend_rust__rust_agrees_where_it_compiles.snap'
REGEN_TESTS='test(export_the_corpus_for_the_site) + test(native_agrees_where_it_compiles) + test(rust_agrees_where_it_compiles)'
mkdir -p "$LOG_DIR"
cd "$REPO"  # never run with cwd inside a worktree this script may remove

MODE="${1:?usage: land-lane.sh land <issue-number>...}"
shift

worker_free() { # $1: dir that must have no live worker
  # A no-op under simple_dispatch.py (2026-09-11): dispatch runs entirely
  # inside a disposable msb sandbox, never as a long-lived host process
  # sitting in a lane directory, so there is never a matching pgrep hit to
  # find here. Left in place rather than deleted: `land land-patch` reuses
  # this same worktree convention once the patch is materialized below, and
  # a future dispatch mechanism reintroducing a real host-side worker
  # process should still get this guard for free.
  for p in $(pgrep -x opencode 2>/dev/null; pgrep -x claude 2>/dev/null); do
    case "$(readlink /proc/$p/cwd 2>/dev/null)" in "$1"*) return 1 ;; esac
  done
  return 0
}

# 8>&- everywhere a child is spawned: fd 8 carries the land flock, and any
# child inheriting it keeps the whole queue locked for its own lifetime -- a
# retriggered WORKER held the lock through its 20-minute run and three lands
# queued behind it (2026-09-01, the same disease as the tick's fd-9 leak).
# The test-gate subshells (just check, cargo nextest, just test) need it too:
# sccache, spawned as RUSTC_WRAPPER down that chain, persists as a daemon after
# them and would hold the lock fd forever (2026-09-07: it stalled the queue
# via inode reuse when land.lock was deleted+recreated).
fire_tick() {
  (nohup "$SCRIPTS/drive-tick.sh" >>"$LOG_DIR/event-ticks.log" 2>&1 &) 8>&-
}

# Red gate or conflict: write evidence into the lane (untracked scratch is
# sanctioned and cleaned on the next land attempt), then either re-dispatch
# the lane's worker with a templated brief or, past the cap, leave the marker.
retrigger() { # $1: issue number  $2: short failure kind  $3: evidence file
  local n=$1 kind=$2 evidence=$3 count brief_file
  count=$(( $(cat "$LOG_DIR/land-retries-issue-$n" 2>/dev/null || echo 0) + 1 ))
  echo "$count" >"$LOG_DIR/land-retries-issue-$n"
  if [ "$count" -gt "$RETRY_CAP" ]; then
    echo "landing issue-$n: $kind, attempt $count -- retry cap reached" \
      >"$LOG_DIR/land-failed-issue-$n"
    echo "[land] issue-$n: $kind on attempt $count -- CAP REACHED, left for escalation"
    return
  fi
  echo "[land] issue-$n: $kind on attempt $count -- re-dispatching via simple_dispatch.py"
  # simple_dispatch.py resets the row fresh (a new disposable clone) on
  # every run, same stateless-per-attempt reasoning as drive-tick.sh's own
  # ticks -- a file left in the OLD worktree would not survive to the
  # retry, so the evidence goes straight into the brief text instead of a
  # copied LAND-FAILURE.txt. Written to plans/simple-briefs/<row>.txt, the
  # SAME convention the coordinator's own fresh dispatches use (not
  # $LOG_DIR) -- simple_dispatch.py requires --brief-dir/<row_id>.txt
  # exactly, and using the real convention means a human or the next tick
  # can find this repair brief the same way as any other.
  brief_file="$REPO/plans/simple-briefs/$n.txt"
  mkdir -p "$REPO/plans/simple-briefs"
  {
    echo "A previous dispatch completed this task, but landing the branch on main"
    echo "FAILED: $kind (landing attempt $count of $((RETRY_CAP + 1))). The exact evidence:"
    echo
    cat "$evidence"
    echo
    echo "Your job now is ONLY to make this branch land: fix whatever the evidence above"
    echo "shows failing, and re-run the verify command yourself. Do not start new feature work."
  } >"$brief_file"
  (cd / && nohup python3 "$SCRIPTS/simple_dispatch.py" "$n" --brief-dir "$REPO/plans/simple-briefs" \
    >>"$LOG_DIR/simple-dispatch-issue-$n.log" 2>&1 &) 8>&-
  # This dispatch is NOT self-landing (simple_dispatch.py deliberately
  # never calls land-lane.sh itself, staying a pure dispatch primitive --
  # see plans/simple-dispatch-design.md) -- the next drive tick is
  # responsible for noticing a fresh GREEN row for "$n" in
  # plans/dispatch-log.csv and running `land-lane.sh land-patch $n
  # <patch-path>` again, the same way it does for any other dispatch.
}

land_one() { # $1: row/issue identifier already checked out at $LANES/issue-$1
  # on branch issue-$1. Returns 0 if it landed on main (pushed), 1 for any
  # other outcome (skipped, deferred, or re-dispatched via retrigger --
  # all already logged by the point they return here).
  local n=$1 d B
  d="$LANES/issue-$n"
  B="issue-$n"
  [ -d "$d" ] || { echo "[land] skip issue-$n: no worktree $d"; return 1; }
  worker_free "$d" || { echo "[land] skip issue-$n: live worker"; return 1; }
  # Untracked work inside a subdirectory is real output a worker cannot rm,
  # not scratch -- stage it before anything else touches this tree. An
  # enumerated directory allowlist here (src/tests/docs/site/plans) silently
  # stopped covering new top-level dirs twice already (issue-168,
  # 2026-09-02: missed src/; benchmark-fasta-build, 2026-09-06: missed
  # benches/, and the untracked-cleanup below would have deleted it before
  # this fix). Root-level loose files are the only sanctioned worker
  # scratch, since workers cannot rm.
  git -C "$d" status --porcelain -z | while IFS= read -r -d '' entry; do
    case "$entry" in
      '?? '*/*) git -C "$d" add -- "${entry#\?\? }" ;;
    esac
  done
  if [ -n "$(git -C "$d" status --porcelain | grep -v '^??')" ]; then
    # Worker exit IS the done signal (maintainer ruling, 2026-09-02,
    # approved interactively): four issue-154 runs produced the right diff
    # and never ran git commit, so a tracked-dirty tree with a green fast
    # check is finished work nobody persisted -- commit it mechanically and
    # land it. A red check means genuinely unfinished: skip, the rebrief
    # path owns it.
    CHECK_LOG="$LOG_DIR/land-autocommit-issue-$n.log"
    (cd "$d" && just check) >"$CHECK_LOG" 2>&1 8>&-
    CHECK_RC=$?
    if [ "$CHECK_RC" -ne 0 ]; then
      # A worktree that has sat through several merges can carry a stale
      # incremental target/ cache that only ever seems to affect these two
      # "which corpus programs compile" snapshots -- real, expected drift
      # whenever the corpus grows, not a regression (confirmed 2026-09-06:
      # a fresh clone of the identical committed state passed clean while
      # the long-lived worktree failed here). Accept only these two
      # known-volatile snapshots and retry once before giving up for real.
      for snap in tests/snapshots/backend_llvm__native_agrees_where_it_compiles.snap \
                  tests/snapshots/backend_rust__rust_agrees_where_it_compiles.snap; do
        [ -f "$d/$snap.new" ] && mv "$d/$snap.new" "$d/$snap"
      done
      (cd "$d" && just check) >"$CHECK_LOG" 2>&1 8>&-
      CHECK_RC=$?
    fi
    if [ "$CHECK_RC" -eq 0 ]; then
      git -C "$d" add -u
      git -C "$d" commit -q -m "Auto-commit worker output for gh:$n (green tree at exit)

The worker exited leaving these tracked changes uncommitted with just
check green; land-lane.sh persisted them mechanically (maintainer
ruling, 2026-09-02: a worker exit is the done signal, the script owns
persistence).

Written by the lane worker; committed by land-lane.sh."
      echo "[land] issue-$n: auto-committed a green dirty tree"
    else
      echo "[land] skip issue-$n: tracked changes with a RED just check (not done)"
      return 1
    fi
  fi
  # Whatever remains untracked now is root-level scratch a worker cannot
  # rm (subdirectory work was staged above, before this could delete it).
  if [ "$(git -C "$d" status --porcelain | grep -c '^??')" -gt 0 ]; then
    git -C "$d" clean -fdq
  fi
  if [ "$(git -C "$REPO" rev-list --count "main..$B")" -eq 0 ]; then
    echo "[land] skip issue-$n: nothing ahead of main"; return 1
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
    CONFLICTED=$(git -C "$PDIR" diff --name-only --diff-filter=U)
    GEN_ONLY=1
    for f in $CONFLICTED; do
      case " $GENERATED " in *" $f "*) ;; *) GEN_ONLY=0 ;; esac
    done
    if [ "$GEN_ONLY" -eq 1 ] && [ -n "$CONFLICTED" ]; then
      # Every conflicted file is generated: take main's copy, rerun the
      # generators, and the merge is resolved without a worker.
      echo "[land] issue-$n: conflicts are generated files only -- regenerating"
      git -C "$PDIR" checkout --ours -- $CONFLICTED
      git -C "$PDIR" add $CONFLICTED
      if (cd "$PDIR" && cargo nextest run -E "$REGEN_TESTS" >/dev/null 2>&1; \
          cargo insta accept >/dev/null 2>&1; \
          cargo nextest run -E "$REGEN_TESTS") 8>&- >>"$GATE_LOG" 2>&1; then
        git -C "$PDIR" add $CONFLICTED
        git -C "$PDIR" commit -q --no-edit -F "$MSG_FILE"
      else
        { echo "MERGE CONFLICT (generated files, but regeneration failed):";
          echo "$CONFLICTED"; } >>"$GATE_LOG" 2>&1
        git -C "$PDIR" merge --abort 2>/dev/null || true
        cleanup_tmp
        retrigger "$n" "merge conflict with main (regeneration failed)" "$GATE_LOG"
        return 1
      fi
    else
      { echo "MERGE CONFLICT merging origin/main + this branch:";
        echo "$CONFLICTED"; } >>"$GATE_LOG" 2>&1
      git -C "$PDIR" merge --abort 2>/dev/null || true
      cleanup_tmp
      retrigger "$n" "merge conflict with main" "$GATE_LOG"
      return 1
    fi
  fi
  if ! (cd "$PDIR" && just test) >>"$GATE_LOG" 2>&1 8>&-;then
    tail -n 60 "$GATE_LOG" >"$GATE_LOG.tail" && mv "$GATE_LOG.tail" "$GATE_LOG"
    cleanup_tmp
    retrigger "$n" "the full test suite went red" "$GATE_LOG"
    return 1
  fi

  # Green: land the tested result. Bounded retry around a busy tick's
  # board commit in the main checkout; lane branches never touch plans/,
  # so a moved main cannot conflict here. 36x5s, not 12x5s: a tick session
  # keeps board.yaml dirty for its whole multi-minute run, and a green
  # 155 land burned its entire 60s window against one and deferred
  # (2026-09-01) -- three minutes spans a typical tick end.
  ok=0
  for _ in $(seq 36); do
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
    return 1
  fi
  git -C "$REPO" push
  git worktree remove --force "$d" 2>/dev/null || git worktree remove "$d"
  git branch -d "$B" 2>/dev/null || true
  rm -f "$LOG_DIR/land-retries-issue-$n" "$LOG_DIR/land-failed-issue-$n" \
        "$LOG_DIR/escalated-issue-$n" "$LOG_DIR/investigating-issue-$n" \
        "$MSG_FILE" "$GATE_LOG"
  echo "[land] issue-$n -> main: $(git -C "$REPO" log --merges --format=%s -1) (pushed)"
  return 0
}

case "$MODE" in
land)
  [ $# -ge 1 ] || { echo "no issues given" >&2; exit 2; }
  # One land at a time, machine-wide. Bounded wait with an explicit give-up
  # (house pattern): the periodic tick is the backstop that re-fires a land
  # that gave up here.
  # A path under $LOG_DIR, not /tmp: sccache (unrelated to this pipeline) once
  # ended up holding an flock on /tmp/toylang-land.lock via inode reuse in
  # that high-churn shared directory, silently stalling the whole queue for
  # 10+ minutes (2026-09-06). Nothing else touches $LOG_DIR.
  exec 8>"$LOG_DIR/land.lock"
  flock -w 1800 8 || { echo "[land] queue lock held 30+ min -- gave up (tick will retry)" >&2; fire_tick; exit 1; }
  ANY_GREEN=0
  for n in "$@"; do
    land_one "$n" && ANY_GREEN=1
  done
  # One tick per invocation: board-archive moves and post-land review on
  # green, escalation routing on failure, rebrief logic when nothing landed.
  fire_tick
  [ "$ANY_GREEN" -eq 1 ] || exit 1
  ;;
land-patch)
  # simple_dispatch.py (2026-09-11: the only dispatch mechanism) never
  # creates a lane worktree -- it produces a plain patch file from a
  # disposable clone, extracted via `git format-patch`. This mode is the
  # adapter: materialize the SAME worktree/branch convention `land_one`
  # already expects from one, by applying the patch onto a fresh branch
  # off main, then fall through to the exact same proven landing logic
  # unchanged. `git am`, not `git apply` + a manual commit: the patch
  # already carries real author/commit-message metadata from
  # format-patch, and `am` preserves it instead of flattening to a
  # synthetic commit.
  [ $# -eq 2 ] || { echo "usage: land-lane.sh land-patch <row-id> <patch-file>" >&2; exit 2; }
  n=$1; patch_file=$2
  [ -f "$patch_file" ] || { echo "[land] land-patch $n: no such patch file $patch_file" >&2; exit 2; }
  exec 8>"$LOG_DIR/land.lock"
  flock -w 1800 8 || { echo "[land] queue lock held 30+ min -- gave up (tick will retry)" >&2; fire_tick; exit 1; }
  d="$LANES/issue-$n"
  B="issue-$n"
  git worktree remove --force "$d" 2>/dev/null || true
  git branch -D "$B" 2>/dev/null || true
  git worktree add -b "$B" "$d" main -q
  AM_LOG="$LOG_DIR/land-patch-am-issue-$n.log"
  if ! git -C "$d" am "$patch_file" >"$AM_LOG" 2>&1; then
    # The patch was generated against whatever commit was HEAD when
    # simple_dispatch.py cloned -- main has very likely moved since. A
    # clean git-am failure here means real drift, not a bug in the patch
    # itself; route it through the SAME retry/escalation path as any
    # other landing failure rather than a bespoke one.
    git -C "$d" am --abort 2>/dev/null || true
    git worktree remove --force "$d" 2>/dev/null || true
    git branch -D "$B" 2>/dev/null || true
    retrigger "$n" "patch from simple_dispatch.py did not apply cleanly onto current main" "$AM_LOG"
    fire_tick
    exit 1
  fi
  ANY_GREEN=0
  land_one "$n" && ANY_GREEN=1
  fire_tick
  [ "$ANY_GREEN" -eq 1 ] || exit 1
  ;;
fold | promote | wip)
  echo "the accumulator pipeline is retired (2026-09-01): use land-lane.sh land <issue>" >&2
  exit 2 ;;
*)
  echo "unknown mode: $MODE" >&2; exit 2 ;;
esac
