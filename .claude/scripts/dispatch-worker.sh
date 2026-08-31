#!/usr/bin/env bash
# Enwiro-free delegation (maintainer simplification, 2026-08-30): a lane is just a
# git worktree under ~/.local/share/toylang-lanes plus a background opencode worker.
# No env, no workspace, no focus dance -- observability is the live log this prints.
#
# Usage: dispatch-worker.sh <issue-number> '<task-specific brief>'
# Continuation dispatches reuse the existing worktree (never with a live worker).
#
# The standard build-brief boilerplate (role, AGENTS.md, gates, constraints,
# KNOWN DENIALS, escalation) is wrapped around the passed text HERE, so the
# dispatcher writes only the task-specific sentences: pointers to the code, the
# ruling, prior-run root causes. BRIEF_RAW=1 skips the wrapping for briefs with
# a different shape (research dispatches, custom continuations).
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
# Lanes cut from the LARGEST live accumulator when one exists (landed-but-
# unpromoted work must be buildable-upon; size-driven pipeline, 2026-08-30),
# from origin/main otherwise.
BASE=origin/main
BEST=-1
for b in $(git -C "$REPO" for-each-ref --format='%(refname:short)' 'refs/heads/to-merge-*'); do
  s=$(git -C "$REPO" diff --shortstat "main...$b" 2>/dev/null \
    | { grep -oE '[0-9]+ (insertion|deletion)' || true; } | awk '{s+=$1} END {print s+0}')  # awk not bc: bc absent here
  [ "${s:-0}" -gt "$BEST" ] && { BASE="$b"; BEST=$s; }
done
if [ -d "$d" ]; then
  echo "[dispatch] continuing in existing lane $d"
else
  # -B would steal a branch checked out elsewhere; add fails loudly instead,
  # which is the guard we want (that task already has a worktree). The
  # fallback here USED to silently reuse whatever branch already existed --
  # an orphaned issue-155 branch from an abandoned gh:136 dispatch got
  # silently inherited by a later, unrelated gh:155 dispatch this way (and
  # the same for issue-157), so the worker exited having never touched its
  # real task. Fail loudly instead: a same-name branch with no worktree is
  # always leftover state, never a legitimate continuation path (that's
  # what the [ -d "$d" ] branch above is for).
  if ! git -C "$REPO" worktree add -b "issue-$N" "$d" "$BASE" -q 2>/dev/null; then
    echo "refusing: branch issue-$N already exists with no worktree -- orphaned from an" \
      "earlier dispatch (contaminated issue-155/issue-157 this way, 2026-08-31)." \
      "Inspect it first (git log issue-$N), then either delete the stale branch or" \
      "dispatch this task under a different lane name." >&2
    exit 1
  fi
fi

if [ -z "${BRIEF_RAW:-}" ]; then
  BRIEF="You are a delegated worker for the toylang repository, in this git worktree on branch issue-$N. FIRST read AGENTS.md at the worktree root and follow it throughout, including its commit rules. Your task is GitHub issue #$N: run \`gh issue view $N\` and read every comment before touching anything. $BRIEF While iterating, test with \`just check\` (the fast loop); \`just --list\` names every repo task. Definition of done: implementation complete; \`just check\` green from the worktree root (a cold worktree compiles first -- give it time, never abort it; the full \`just test\` runs at promotion, not per lane); \`just fmt\` and \`just clippy\` clean on code you touched; work committed on this branch per AGENTS.md with the provenance line \"Written by DeepSeek V4 Flash via opencode.\". Hard constraints: work ONLY inside this worktree; do NOT push; do not touch plans/. If a command is denied, adapt with an allowed alternative rather than retrying. KNOWN DENIALS: gh issue list/search (only gh issue view <N> is allowed), rm and file deletion, environment-variable-prefixed commands (VAR=x cmd never matches the allow-list -- run the plain command; for snapshot updates run the plain test so insta writes .snap.new files, then cargo insta accept), shell loop constructs (while/for), heredocs, bash file writes (> and >> redirection), writes under /tmp, and multi-command bash lines are always denied -- CREATE AND EDIT FILES ONLY WITH YOUR write/edit TOOLS, one simple command per bash call, get file lists with one grep/rg/ls call and handle files one per tool call, and run scripts as \`python3 script.py\` after writing them with the write tool. Scratch files you cannot delete are FINE: simply do not git add them, commit your real work, and exit -- never let cleanup failures stop you from committing. If something needs deciding that this brief does not settle, write ESCALATION.md at the worktree root -- the question, the alternatives, their costs -- COMMIT it on this branch, take the most conservative continuation, and keep going; never stop to wait for a human. If you genuinely cannot continue at all, ESCALATION.md plus exiting IS the correct done-state -- never commit broken work to look finished."
fi

LIVE="$LOG_DIR/issue-$N.live.log"
(cd "$d" && nohup "$REPO/.claude/scripts/opencode-worker.sh" "$BRIEF" \
  >>"$LIVE" 2>&1 &)
echo "[dispatch] issue-$N worker launched; watch: tail -f $LIVE"
