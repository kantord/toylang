#!/usr/bin/env bash
# Stateless-ish drive tick. One coordinator session is REUSED across ticks (--resume)
# so the prompt cache stays warm and the skill/board context is not re-read every ten
# minutes; the loop observes the session's context size from outside (the usage block
# of the JSON result) and simply starts a fresh session once it crosses MAX_CONTEXT --
# externally-enforced amnesia instead of in-session compaction. Ticks are drilled to
# trust disk over memory, so a reset never loses state.
#
# Runs in auto permission mode -- the same classifier guardrail interactive sessions
# get. Every tick runs sonnet (maintainer rule, 2026-08-30): landing is mostly
# plumbing, and review panels/subagents are retired outright (same-day ruling) --
# the coordinator reads diffs itself. `audit` as $1 runs the audit prompt.
set -uo pipefail
REPO=/home/kantord/repos/toylang
WORKTREES=/home/kantord/.local/share/enwiro/worktrees/pr/toylang-1234138d
LANES="$HOME/.local/share/toylang-lanes"  # enwiro-free lanes (dispatch-worker.sh)
LOG_DIR="$HOME/.cache/toylang-drive"
SID_FILE="$LOG_DIR/session-id"
MAX_CONTEXT="${MAX_CONTEXT:-90000}"
mkdir -p "$LOG_DIR"

# Never two ticks at once: a landing tick can outlive several loop intervals.
exec 9>/tmp/toylang-drive-tick.lock
flock -n 9 || { echo "[drive-tick] $(date '+%H:%M:%S') another tick holds the lock (event-driven landing, most likely) -- yielded"; exit 0; }

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
STATE=""
# A delegated row names its worktree either by pool lane (lane: lane-N, the
# gh:124 worker pool -- resolved through the stable enwiro env symlink) or by
# issue number (the classic one-env-per-issue flow).
DELEGATED=$(python3 -c "
import yaml
for r in yaml.safe_load(open('plans/board.yaml')):
    if r.get('status') != 'delegated':
        continue
    if r.get('lane'):
        print('lane:' + r['lane'])
    elif str(r.get('issue', '')).startswith('gh:'):
        print('issue-' + r['issue'][3:])
" | tr '\n' ' ')
for wt in $DELEGATED; do
  case "$wt" in
    lane:*) d="$HOME/.enwiro_envs/toylang@${wt#lane:}/toylang@${wt#lane:}" ;;
    *) d="$LANES/$wt"; [ -d "$d" ] || d="$WORKTREES/$wt" ;;
  esac
  [ -d "$d" ] || { TRIGGER="lane $wt has no worktree"; continue; }
  ahead=$(git -C "$d" rev-list --count main..HEAD 2>/dev/null || echo 0)
  dirty=$(git -C "$d" status --porcelain 2>/dev/null | wc -l)
  recent8=$(find "$d" -name .git -prune -o -name target -prune -o -type f \
    -newermt '-8 minutes' -print -quit 2>/dev/null)
  recent30=$(find "$d" -name .git -prune -o -name target -prune -o -type f \
    -newermt '-30 minutes' -print -quit 2>/dev/null)
  commit_age=$(( $(date +%s) - $(git -C "$d" log -1 --format=%ct 2>/dev/null || echo 0) ))
  live=0
  d_real=$(readlink -f "$d")  # /proc cwd is resolved; $d may be the env symlink
  for p in $(pgrep -x claude 2>/dev/null; pgrep -x opencode 2>/dev/null); do
    # opencode workers are the delegation default (rollout ruling, 2026-08-30);
    # claude matches cover in-flight pre-ruling lanes until they land.
    case "$(readlink /proc/$p/cwd 2>/dev/null)" in "$d_real"*) live=1 ;; esac
  done
  esc=""; [ -f "$d/ESCALATION.md" ] && esc=" ESCALATION.md"; [ -f "$d/RESEARCH.md" ] && esc="$esc RESEARCH.md"
  # Failure streak: every run leaves a timestamped event log, so N logs with
  # zero commits ahead IS an N-failure streak -- crash-proof, no state file.
  # (issue-133 died five times on one sandbox edge, 2026-08-30, and every tick
  # saw only "dead lane" with no memory that this was death #5.)
  runs=$(ls "$LOG_DIR/opencode/"*"-$wt.jsonl" 2>/dev/null | wc -l)
  STATE="$STATE [$wt: ahead=$ahead dirty=$dirty live=$live commit_age=${commit_age}s runs=$runs$esc]"
  if [ "$ahead" -gt 0 ] && [ "$dirty" -eq 0 ] && [ "$live" -eq 0 ]; then
    # A gone worker with committed clean work is done NOW -- opencode workers
    # exit on finish (event-driven landing, 2026-08-30), so no quiet window.
    TRIGGER="lane $wt looks landable (worker exited)"
  elif [ "$ahead" -gt 0 ] && [ "$dirty" -eq 0 ] && [ -z "$recent8" ] && [ "$commit_age" -ge 480 ]; then
    # A LIVE session that has gone quiet still needs the 8-minute window (a
    # claude-era lane may idle after finishing). Both quiet signals matter:
    # committing touches no working-tree mtimes, so a lane that edits, tests
    # for ten minutes, then commits looks file-quiet.
    TRIGGER="lane $wt looks landable"
  elif [ "$live" -eq 0 ] && [ "$ahead" -eq 0 ] && [ "$runs" -ge 4 ]; then
    # Streak cap: stop feeding the lane. Escalate to the maintainer's mail ONCE
    # (the marker suppresses re-noise); their answer or a landing clears it.
    if [ ! -f "$LOG_DIR/escalated-$wt" ]; then
      TRIGGER="lane $wt: $runs commitless runs -- STOP redispatching; escalate to the maintainer inbox"
    fi
  elif [ "$live" -eq 0 ] && [ "$ahead" -eq 0 ] && [ "$runs" -ge 2 ]; then
    TRIGGER="${TRIGGER:-lane $wt: $runs commitless runs -- diagnose the last event log and REBRIEF; never repeat a failed brief}"
  elif [ "$live" -eq 0 ]; then
    TRIGGER="${TRIGGER:-lane $wt has no live worker}"
  elif [ -z "$recent30" ] && [ "$commit_age" -ge 1800 ]; then
    TRIGGER="${TRIGGER:-lane $wt silent 30+ minutes (stall diagnosis)}"
  fi
done
# Orphaned commits: a worktree ahead of main whose row is NOT delegated is
# forgotten work (post-landing hook growth is the known producer). Pool lane
# worktrees (gh:124) live under a different base and are cooked as
# <lane>-<8 hex>, so both trees get swept.
POOL_WORKTREES="$HOME/.local/share/enwiro/worktrees/toylang-1234138d"
for d in "$WORKTREES"/*/ "$POOL_WORKTREES"/*/ "$LANES"/*/; do
  [ -d "$d" ] || continue
  wt=$(basename "$d")
  lane=$(printf '%s' "$wt" | sed -E 's/-[0-9a-f]{8}$//')
  case " $DELEGATED " in *" $wt "* | *" lane:$lane "*) continue ;; esac
  ahead=$(git -C "$d" rev-list --count main..HEAD 2>/dev/null || echo 0)
  [ "$ahead" -gt 0 ] && TRIGGER="${TRIGGER:-non-delegated worktree $wt is $ahead ahead of main}"
done
# Accumulators (to-merge-* branches, the size-driven pipeline): a full one
# promotes itself at fold time, so the tick only has to catch STALENESS
# (untouched 30+ min -> promote as-is, straight to main) and red promotions.
for b in $(git for-each-ref --format='%(refname:short)' 'refs/heads/to-merge-*'); do
  lines=$(git diff --shortstat "main...$b" 2>/dev/null | grep -oE '[0-9]+ insertion|[0-9]+ deletion' | grep -oE '[0-9]+' | paste -sd+ | bc 2>/dev/null || echo 0)
  age=$(( $(date +%s) - $(git log -1 --format=%ct "$b" 2>/dev/null || date +%s) ))
  promoting=""; [ -f "$LOG_DIR/promoting-$b" ] && promoting=" promoting"
  STATE="$STATE [$b: lines=${lines:-0} age=${age}s$promoting]"
  if [ -f "$LOG_DIR/promote-failed-$b" ]; then
    TRIGGER="${TRIGGER:-promotion of $b went RED (main untouched) -- route the repair}"
  elif [ -z "$promoting" ] && [ "$age" -ge 1800 ]; then
    TRIGGER="${TRIGGER:-$b untouched ${age}s -- stale, promote it as-is (land-lane.sh promote, detached)}"
  fi
done
# Maintainer input always runs the tick (the 5-minute quiet rule is judged inside).
if python3 -c "
import json, sys
for f in ('docs/.annotations/inbox.json', 'docs/.annotations/notes.json'):
    d = json.load(open(f))
    if d.get('records') or d.get('composed'):
        sys.exit(0)
sys.exit(1)" 2>/dev/null || [ -n "$(ls docs/.grill/ 2>/dev/null | grep -v '\.round\.yaml$')" ]; then
  # Outgoing *.round.yaml files WAIT on the maintainer -- only submissions and
  # annotation records count as input.
  TRIGGER="${TRIGGER:-maintainer input pending}"
fi
# Decide starvation: the maintainer keeps checking an empty inbox while decide
# rows sit ready. NOT a fallback (it starved twice, 2026-08-30: as a fallback it
# lost to every landing and dead-lane trigger, and the maintainer drained both
# buffered rounds in ten minutes with nothing refilling) -- an under-filled round
# buffer ALWAYS joins the trigger, alongside whatever else the tick has.
if [ "$(ls docs/.grill/*.round.yaml 2>/dev/null | wc -l)" -lt 2 ]; then
  # Keep TWO rounds buffered (maintainer flow, 2026-08-30): grilling happens WHILE
  # workers grind, so finishing one round must always reveal the next, not a wait.
  STARVE=$(python3 -c "
import yaml
rows = yaml.safe_load(open('plans/board.yaml'))
live = {r['id'] for r in rows}
ready = [r['id'] for r in rows
         if r.get('status') == 'todo' and r.get('kind') == 'decide'
         and all(n not in live for n in r.get('needs', []))]
if ready:
    print(f'round buffer under-filled with {len(ready)} decide rows ready -- compose a grill round')" 2>/dev/null)
  [ -n "$STARVE" ] && TRIGGER="${TRIGGER:+$TRIGGER; }$STARVE"
fi
# A free lane with a ready row means dispatch is due.
if [ -z "$TRIGGER" ]; then
  TRIGGER=$(python3 -c "
import yaml
rows = yaml.safe_load(open('plans/board.yaml'))
live = {r['id'] for r in rows}  # a needs id absent here landed and archived (issue #113)
lanes = sum(1 for r in rows if r.get('status') == 'delegated' and r.get('kind') == 'build')
ready = [r['id'] for r in rows
         if r.get('status') == 'todo' and r.get('kind') == 'build'
         and all(n not in live for n in r.get('needs', []))]
if lanes < 8 and ready:
    print(f'{8 - lanes} free lanes, ready: {\" \".join(ready[:3])}')" 2>/dev/null)
fi
# Exhaustion: nothing delegated, nothing ready to build -- the idle exception
# (drive skill) lets the tick self-originate one or two exploration rows.
if [ -z "$TRIGGER" ]; then
  TRIGGER=$(python3 -c "
import yaml
rows = yaml.safe_load(open('plans/board.yaml'))
live = {r['id'] for r in rows}
lanes = sum(1 for r in rows if r.get('status') == 'delegated')
ready = sum(1 for r in rows if r.get('status') == 'todo' and r.get('kind') == 'build'
            and all(n not in live for n in r.get('needs', [])))
if lanes == 0 and ready == 0:
    print('board exhausted -- idle exception: self-originate 1-2 exploration rows (drive skill)')" 2>/dev/null)
fi
[ "${1:-tick}" = "audit" ] && TRIGGER="scheduled audit"
if [ -z "$TRIGGER" ]; then
  echo "[drive-tick] $(date '+%H:%M:%S') nothing to do (workers grinding, no input) -- skipped, zero tokens"
  exit 0
fi

if [ "${1:-tick}" = "audit" ]; then
  PROMPT='Periodic audit (drive skill, "The periodic audit" section) for toylang at /home/kantord/repos/toylang. Reconstruct everything from disk; trust disk over anything remembered from earlier ticks. Check: every open GitHub issue maps to a board row; every delegated row has a live or accounted-for lane; no worktree holds unmerged commits the board thinks landed; no falsely-stuck lanes. Fix what is mechanical, file issues for the rest. End quietly if clean.'
else
  PROMPT='Drive tick (drive skill, monitoring phase) for toylang at /home/kantord/repos/toylang. Reconstruct state from disk -- plans/board.yaml, the worktrees, the annotation stores -- and trust disk over anything remembered from earlier ticks. FIRST, before any landing or dispatch: poll BOTH annotation stores: docs/.annotations/inbox.json AND docs/.annotations/notes.json -- apply entries older than 5 minutes and clear them at capture, EXCEPT records whose page is a docs/.grill/*.round.yaml file: those are wizard submissions, explicit clicks, applied IMMEDIATELY with no quiet period; wizard submissions in docs/.grill/ process immediately (delete round files at capture). Maintainer input outranks every landing. THEN read board rows with status: delegated and check each lane worktree (commits, dirty files, live worker via pgrep cwd). Finished lanes (ahead, clean, worker gone) land via the land-delegated-work skill size-driven flow: diff read, then .claude/scripts/land-lane.sh fold <msgfile> <issues...> (fast gate; a full accumulator fires its own detached promotion) with board-archive.py for the row moves; run land-lane.sh promote <branch> yourself, DETACHED with nohup, only when the trigger names a stale accumulator. After landings, dispatch ready board rows into free lanes (cap 8; decide rows never consume a lane or a ready-set slot) with .claude/scripts/dispatch-worker.sh <N> "<brief>" -- the enwiro-free opencode default (claude delegation retired 2026-08-30; brief shape in the enwiro-delegate skill; record every rollout incident in plans/opencode-rollout.md). If the trigger says the round buffer is under-filled: compose the next wizard round BEFORE any landing -- an empty maintainer inbox outranks lane plumbing (landings and dispatches self-heal on later ticks; the maintainer grilling NOW does not). Read any already-pending round first and never re-ask its questions; keep two rounds buffered so the maintainer can grill back-to-back. Write docs/.grill/<topic>.round.yaml -- 3-5 ready decide rows batched by theme, each option carrying real verified code examples (the maintainer standing rule; delegate substantial example-preparation to a research worker rather than writing it all in-tick). NEVER edit a lane worktree source file, test, or snapshot yourself -- a dead or half-done lane gets a continuation or research dispatch (enwiro-delegate skill), no matter how small the fix looks (three coordinators hand-fixed issue-116 in one evening and each left it messier). FAILURE-STREAK POLICY (the snapshot carries runs= per lane): a lane at 2-3 commitless runs never gets a repeat of a failed brief -- read the LAST event log tail in ~/.cache/toylang-drive/opencode/ for the actual death (usually a permission auto-reject), and redispatch with that root cause spelled out in the brief. At 4+ commitless runs STOP: do not redispatch; write an escalation the maintainer will see in their mail -- a docs/.grill/ escalation-flow round question naming the lane, the repeated root cause, and the options (stronger OPENCODE_MODEL, reshape the task, drop it) -- then touch ~/.cache/toylang-drive/escalated-issue-<N> so the gate stays quiet; rm that marker when acting on the maintainer ruling. BOUND THE TICK: at most one round composition plus one landing, or up to three landings (a cascade counts as one) when no composition is due, then END the session even if more work is visible -- the loop and event ticks continue with fresh sessions; one session serially working a whole backlog blows far past the context ceiling, which is only enforced between ticks. If nothing changed, end quietly without writing anything.'
fi

TS=$(date +%Y%m%d-%H%M%S)
OUT="$LOG_DIR/$TS-${1:-tick}-$MODEL.json"
echo "[drive-tick] $(date '+%H:%M:%S') ${1:-tick} starting on $MODEL -- $TRIGGER (log: $OUT)"
INBOX_N=$(python3 -c "
import json
d=json.load(open('docs/.annotations/inbox.json'))
print(len(d.get('records',[])))" 2>/dev/null || echo '?')
ROUNDS=$(ls docs/.grill/*.round.yaml 2>/dev/null | xargs -rn1 basename | tr '\n' ' ')
PROMPT="$PROMPT Trigger for this tick: $TRIGGER. Pre-computed state snapshot (gathered from disk this second by the gate script -- ACT ON IT instead of re-running board reads, lane checks, and store polls; re-verify only what you are about to modify):${STATE:- no delegated lanes} [inbox_records=$INBOX_N pending_rounds=${ROUNDS:-none}]. You are a ROUTER: your turns are for decisions and the four scripts (dispatch-worker.sh, land-lane.sh, board-archive.py, round files), never for exploration."

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
