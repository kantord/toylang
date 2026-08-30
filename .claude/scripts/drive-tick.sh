#!/usr/bin/env bash
# Stateless-ish drive tick. One coordinator session is REUSED across ticks (--resume)
# so the prompt cache stays warm and the skill/board context is not re-read every ten
# minutes; the loop observes the session's context size from outside (the usage block
# of the JSON result) and simply starts a fresh session once it crosses MAX_CONTEXT --
# externally-enforced amnesia instead of in-session compaction. Ticks are drilled to
# trust disk over memory, so a reset never loses state.
#
# Runs in auto permission mode -- the same classifier guardrail interactive sessions
# get. Model per tick: sonnet for routine monitoring/dispatch, fable when a lane looks
# landable (landing is where review-finding judgment happens); `audit` as $1 always
# runs fable.
set -uo pipefail
REPO=/home/kantord/repos/toylang
WORKTREES=/home/kantord/.local/share/enwiro/worktrees/pr/toylang-1234138d
LOG_DIR="$HOME/.cache/toylang-drive"
SID_FILE="$LOG_DIR/session-id"
MAX_CONTEXT="${MAX_CONTEXT:-90000}"
mkdir -p "$LOG_DIR"

# Never two ticks at once: a landing tick can outlive several loop intervals.
exec 9>/tmp/toylang-drive-tick.lock
flock -n 9 || exit 0

export PATH="$HOME/.local/bin:$HOME/.local/share/pnpm:/usr/local/bin:/usr/bin:/bin"
cd "$REPO"

# The maintainer's mail UI depends on the dev server; revive it if a reboot ate it.
if ! curl -s -o /dev/null --max-time 3 http://localhost:5173/toylang/dev/; then
  (cd "$REPO/site" && nohup pnpm dev --port 5173 --strictPort \
    >>"$LOG_DIR/devserver.log" 2>&1 &)
fi

# Decide in bash whether this tick needs a model at all, and which one. A tick
# runs only for a reason; quiet healthy grinding (workers editing, nothing
# landable, no input) skips at zero token cost. Stuck-ness and orphaned commits
# are TRIGGERS, not things a skip can starve: a dead or 30-minute-silent lane
# and a non-delegated worktree sitting ahead of main both force a run.
MODEL=sonnet
TRIGGER=""
DELEGATED=$(python3 -c "
import yaml
for r in yaml.safe_load(open('plans/board.yaml')):
    if r.get('status') == 'delegated' and str(r.get('issue', '')).startswith('gh:'):
        print('issue-' + r['issue'][3:])
" | tr '\n' ' ')
for wt in $DELEGATED; do
  d="$WORKTREES/$wt"
  [ -d "$d" ] || { TRIGGER="lane $wt has no worktree"; continue; }
  ahead=$(git -C "$d" rev-list --count main..HEAD 2>/dev/null || echo 0)
  dirty=$(git -C "$d" status --porcelain 2>/dev/null | wc -l)
  recent8=$(find "$d" -name .git -prune -o -name target -prune -o -type f \
    -newermt '-8 minutes' -print -quit 2>/dev/null)
  recent30=$(find "$d" -name .git -prune -o -name target -prune -o -type f \
    -newermt '-30 minutes' -print -quit 2>/dev/null)
  commit_age=$(( $(date +%s) - $(git -C "$d" log -1 --format=%ct 2>/dev/null || echo 0) ))
  live=0
  for p in $(pgrep claude 2>/dev/null); do
    case "$(readlink /proc/$p/cwd 2>/dev/null)" in "$d"*) live=1 ;; esac
  done
  if [ "$ahead" -gt 0 ] && [ "$dirty" -eq 0 ] && [ -z "$recent8" ] && [ "$commit_age" -ge 480 ]; then
    # Both quiet signals matter: committing touches no working-tree mtimes, so a
    # lane that edits, tests for ten minutes, then commits looks file-quiet.
    TRIGGER="lane $wt looks landable"
    MODEL=fable
  elif [ "$live" -eq 0 ]; then
    TRIGGER="${TRIGGER:-lane $wt has no live worker}"
  elif [ -z "$recent30" ] && [ "$commit_age" -ge 1800 ]; then
    TRIGGER="${TRIGGER:-lane $wt silent 30+ minutes (stall diagnosis)}"
  fi
done
# Orphaned commits: a worktree ahead of main whose row is NOT delegated is
# forgotten work (post-landing hook growth is the known producer).
for d in "$WORKTREES"/*/; do
  wt=$(basename "$d")
  case " $DELEGATED " in *" $wt "*) continue ;; esac
  ahead=$(git -C "$d" rev-list --count main..HEAD 2>/dev/null || echo 0)
  [ "$ahead" -gt 0 ] && TRIGGER="${TRIGGER:-non-delegated worktree $wt is $ahead ahead of main}"
done
# Maintainer input always runs the tick (the 5-minute quiet rule is judged inside).
if python3 -c "
import json, sys
for f in ('docs/.annotations/inbox.json', 'docs/.annotations/notes.json'):
    d = json.load(open(f))
    if d.get('records') or d.get('composed'):
        sys.exit(0)
sys.exit(1)" 2>/dev/null || [ -n "$(ls docs/.grill/ 2>/dev/null)" ]; then
  TRIGGER="${TRIGGER:-maintainer input pending}"
fi
# A free lane with a ready row means dispatch is due.
if [ -z "$TRIGGER" ]; then
  TRIGGER=$(python3 -c "
import yaml
rows = yaml.safe_load(open('plans/board.yaml'))
done = {r['id'] for r in rows if r.get('status') == 'done'}
lanes = sum(1 for r in rows if r.get('status') == 'delegated' and r.get('kind') == 'build')
ready = [r['id'] for r in rows
         if r.get('status') == 'todo' and r.get('kind') == 'build'
         and all(n in done for n in r.get('needs', []))]
if lanes < 5 and ready:
    print(f'{5 - lanes} free lanes, ready: {\" \".join(ready[:3])}')" 2>/dev/null)
fi
[ "${1:-tick}" = "audit" ] && TRIGGER="scheduled audit"
if [ -z "$TRIGGER" ]; then
  echo "[drive-tick] $(date '+%H:%M:%S') nothing to do (workers grinding, no input) -- skipped, zero tokens"
  exit 0
fi

if [ "${1:-tick}" = "audit" ]; then
  MODEL=fable
  PROMPT='Periodic audit (drive skill, "The periodic audit" section) for toylang at /home/kantord/repos/toylang. Reconstruct everything from disk; trust disk over anything remembered from earlier ticks. Check: every open GitHub issue maps to a board row; every delegated row has a live or accounted-for lane; no worktree holds unmerged commits the board thinks landed; no falsely-stuck lanes. Fix what is mechanical, file issues for the rest. End quietly if clean.'
else
  PROMPT='Drive tick (drive skill, monitoring phase) for toylang at /home/kantord/repos/toylang. Reconstruct state from disk -- plans/board.yaml, the worktrees, the annotation stores -- and trust disk over anything remembered from earlier ticks. Read board rows with status: delegated and check each lane worktree (commits vs main, dirty files, live worker via pgrep cwd). Poll BOTH annotation stores: docs/.annotations/inbox.json AND docs/.annotations/notes.json -- apply entries older than 5 minutes and clear them at capture; wizard submissions in docs/.grill/ process immediately (delete round files at capture). If a lane is finished (ahead of main, clean, 8+ minutes quiet or worker gone), run the land-delegated-work skill -- cascade if 3+ are ready. After landings, dispatch ready board rows into free lanes (cap 5, model per row: haiku/sonnet/fable). If nothing changed, end quietly without writing anything.'
fi

TS=$(date +%Y%m%d-%H%M%S)
OUT="$LOG_DIR/$TS-${1:-tick}-$MODEL.json"
echo "[drive-tick] $(date '+%H:%M:%S') ${1:-tick} starting on $MODEL -- $TRIGGER (log: $OUT)"
PROMPT="$PROMPT Trigger for this tick: $TRIGGER."

run_tick() { # $@: extra claude args (--resume <id> or nothing)
  # stream-json + the colorizer keeps the loop terminal a live, readable trace;
  # the colorizer writes the final result event to $OUT for the context watch.
  claude -p --model "$MODEL" --permission-mode auto \
    --output-format stream-json --verbose \
    "$@" "$PROMPT" 2>>"$LOG_DIR/errors.log" \
    | python3 "$REPO/.claude/scripts/tick-stream.py" "$OUT"
}

SID=""
[ -f "$SID_FILE" ] && SID=$(cat "$SID_FILE")
if [ -n "$SID" ]; then
  run_tick --resume "$SID" || { rm -f "$SID_FILE"; SID=""; }
fi
[ -n "$SID" ] || run_tick

# Observe the context from outside: keep the session while it is small, drop it
# (fresh session next tick) once it crosses MAX_CONTEXT. The result JSON's usage
# is cumulative across turns, so the real context size is the LAST usage entry in
# the session transcript: what the final request actually carried.
python3 - "$OUT" "$SID_FILE" "$MAX_CONTEXT" <<'EOF'
import json, sys, os, glob
out, sid_file, max_ctx = sys.argv[1], sys.argv[2], int(sys.argv[3])
try:
    sid = json.load(open(out)).get("session_id")
except Exception:
    sys.exit(0)
ctx = 0
paths = glob.glob(os.path.expanduser(f"~/.claude/projects/*/{sid}.jsonl"))
for line in open(paths[0]) if paths else []:
    try:
        u = json.loads(line).get("message", {}).get("usage")
    except Exception:
        continue
    if u:
        ctx = sum(u.get(k, 0) for k in
                  ("input_tokens", "cache_read_input_tokens", "cache_creation_input_tokens"))
if sid and 0 < ctx < max_ctx:
    open(sid_file, "w").write(sid)
    keep = True
else:
    try: os.remove(sid_file)
    except FileNotFoundError: pass
    keep = False
print(f"session {sid} context~{ctx} keep={keep}")
EOF
