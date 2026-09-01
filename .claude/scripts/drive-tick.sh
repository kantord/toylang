#!/usr/bin/env bash
# Stateless drive tick: every tick is a brand-new claude -p session, no --resume.
# Ticks are drilled to trust disk over memory, so a fresh session never loses state
# -- resuming across ticks was a prompt-cache optimization only (measured saving:
# ~1-1.5k tokens/tick, well under a cent/tick), and the two flakiest ticks of
# 2026-08-31 (a 90+ minute lock stall, a 0-turn/25ms empty result) both happened on
# a resumed session; every fresh-session tick that night did clean, verifiable work.
# Dropped for reliability -- see plans/board.yaml and the drive skill for the ruling.
#
# Runs in auto permission mode -- the same classifier guardrail interactive sessions
# get. Every tick runs sonnet (maintainer rule, 2026-08-30): landing is mostly
# plumbing, and review panels/subagents are retired outright (same-day ruling) --
# the coordinator reads diffs itself. `audit` as $1 runs the audit prompt.
set -uo pipefail
# PAUSED during the 2026-09-01 landing-pipeline refactor -- remove with the rewrite.
[ -f "$HOME/.cache/toylang-drive/tick-paused" ] && { echo "[drive-tick] paused for refactor"; exit 0; }
REPO=/home/kantord/repos/toylang
WORKTREES=/home/kantord/.local/share/enwiro/worktrees/pr/toylang-1234138d
LANES="$HOME/.local/share/toylang-lanes"  # enwiro-free lanes (dispatch-worker.sh)
LOG_DIR="$HOME/.cache/toylang-drive"
mkdir -p "$LOG_DIR"

# Never two ticks at once: a landing tick can outlive several loop intervals.
exec 9>/tmp/toylang-drive-tick.lock
flock -n 9 || { echo "[drive-tick] $(date '+%H:%M:%S') another tick holds the lock (event-driven landing, most likely) -- yielded"; exit 0; }

export PATH="$HOME/.local/bin:$HOME/.local/share/pnpm:/usr/local/bin:/usr/bin:/bin"
cd "$REPO"

# The maintainer's mail UI depends on the dev server; revive it if a reboot ate it.
# 9>&- : the dev server outlives this script, so it must never inherit the tick
# lock fd -- otherwise it pins the lock open forever and every future tick yields.
if ! curl -s -o /dev/null --max-time 3 http://localhost:5173/toylang/dev/; then
  (cd "$REPO/site" && nohup pnpm dev --port 5173 --strictPort \
    >>"$LOG_DIR/devserver.log" 2>&1 9>&- &)
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
# Dead-lane triggers are picked by STALEST, not first-in-file-order: a soft-default
# TRIGGER="${TRIGGER:-...}" inside this loop let whichever lane sorts first in
# board.yaml claim every tick it was also dead, starving lanes later in the file
# even past their own rebrief/escalate threshold (issue-153 sat 22h at runs=2
# while issue-151, first in the file, kept re-claiming the slot, 2026-08-31).
# DEAD_PRIORITY ranks tiers (3=escalate, 2=rebrief, 1=no-live-worker, 0=stalled-live);
# ties break on commit_age, so the actually-oldest dead lane wins within a tier.
DEAD_PRIORITY=-1
DEAD_TRIGGER=""
DEAD_AGE=-1
for wt in $DELEGATED; do
  case "$wt" in
    lane:*) d="$HOME/.enwiro_envs/toylang@${wt#lane:}/toylang@${wt#lane:}" ;;
    *) d="$LANES/$wt"; [ -d "$d" ] || d="$WORKTREES/$wt" ;;
  esac
  [ -d "$d" ] || { TRIGGER="delegated row $wt has no worktree -- it landed (verify in main log, then board-archive the row)"; continue; }
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
    if [ ! -f "$LOG_DIR/escalated-$wt" ] && [ 3 -gt "$DEAD_PRIORITY" ]; then
      DEAD_PRIORITY=3; DEAD_AGE=$commit_age
      DEAD_TRIGGER="lane $wt: $runs commitless runs -- STOP redispatching; escalate to the maintainer inbox"
    fi
  elif [ "$live" -eq 0 ] && [ "$ahead" -eq 0 ] && [ "$runs" -ge 2 ]; then
    if [ 2 -gt "$DEAD_PRIORITY" ] || { [ 2 -eq "$DEAD_PRIORITY" ] && [ "$commit_age" -gt "$DEAD_AGE" ]; }; then
      DEAD_PRIORITY=2; DEAD_AGE=$commit_age
      DEAD_TRIGGER="lane $wt: $runs commitless runs -- diagnose the last event log and REBRIEF; never repeat a failed brief"
    fi
  elif [ "$live" -eq 0 ]; then
    if [ 1 -gt "$DEAD_PRIORITY" ] || { [ 1 -eq "$DEAD_PRIORITY" ] && [ "$commit_age" -gt "$DEAD_AGE" ]; }; then
      DEAD_PRIORITY=1; DEAD_AGE=$commit_age
      DEAD_TRIGGER="lane $wt has no live worker"
    fi
  elif [ -z "$recent30" ] && [ "$commit_age" -ge 1800 ]; then
    if [ 0 -gt "$DEAD_PRIORITY" ] || { [ 0 -eq "$DEAD_PRIORITY" ] && [ "$commit_age" -gt "$DEAD_AGE" ]; }; then
      DEAD_PRIORITY=0; DEAD_AGE=$commit_age
      DEAD_TRIGGER="lane $wt silent 30+ minutes (stall diagnosis)"
    fi
  fi
done
# Landable lanes (unconditional TRIGGER above) always outrank a dead lane or a
# blocked accumulator; the DEAD_TRIGGER backfill happens once, after the
# accumulator loop below has also had a chance to raise DEAD_PRIORITY -- doing
# it here instead would lock in a low-priority lane trigger before a RED
# promotion or an over-limit accumulator (tiers 4-6) ever got compared.
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
# Landing failures (serial queue, 2026-09-01): land-lane.sh handles its own
# conflict/red re-dispatches (cap 2); a marker here means the cap is spent (or
# the main checkout stayed busy) and the tick must route it. Tier 6 so a
# blocked landing of finished work always outranks routine lane chatter (the
# lesson of the accumulator era: promotion triggers starved behind dead-lane
# rebriefs all night, 2026-08-31).
for m in "$LOG_DIR"/land-failed-issue-*; do
  [ -f "$m" ] || continue
  n=$(basename "$m"); n=${n#land-failed-issue-}
  STATE="$STATE [land-failed: issue-$n -- $(cat "$m")]"
  if [ 6 -gt "$DEAD_PRIORITY" ]; then
    DEAD_PRIORITY=6; DEAD_AGE=0
    DEAD_TRIGGER="landing of issue-$n is stuck ($(cat "$m")) -- route it"
  fi
done
[ -z "$TRIGGER" ] && [ -n "$DEAD_TRIGGER" ] && TRIGGER="$DEAD_TRIGGER"
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
# A free lane with a ready row means dispatch is due. This JOINS the trigger
# instead of being a fallback: as a fallback it starved 2h behind the
# streak/starvation triggers while 7 lanes sat idle (2026-08-30).
DISPATCH=$(python3 -c "
import yaml
rows = yaml.safe_load(open('plans/board.yaml'))
live = {r['id'] for r in rows}  # a needs id absent here landed and archived (issue #113)
lanes = sum(1 for r in rows if r.get('status') == 'delegated' and r.get('kind') == 'build')
ready = [r['id'] for r in rows
         if r.get('status') == 'todo' and r.get('kind') == 'build'
         and all(n not in live for n in r.get('needs', []))]
if lanes < 8 and ready:
    print(f'{8 - lanes} free lanes, ready: {\" \".join(ready[:3])}')" 2>/dev/null)
[ -n "$DISPATCH" ] && TRIGGER="${TRIGGER:+$TRIGGER; }$DISPATCH"
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

# The POLICY is sent fresh every tick (no cross-tick resume). Keep it
# apostrophe-free -- it sits in single quotes.
if [ "${1:-tick}" = "audit" ]; then
  POLICY='Periodic audit (drive skill, "The periodic audit" section) for toylang at /home/kantord/repos/toylang. Reconstruct everything from disk; trust disk over anything remembered from earlier ticks. Check: every open GitHub issue maps to a board row; every delegated row has a live or accounted-for lane; no worktree holds unmerged commits the board thinks landed; no falsely-stuck lanes. Fix what is mechanical, file issues for the rest. End quietly if clean.'
else
  POLICY='Drive tick (drive skill, monitoring phase) for toylang at /home/kantord/repos/toylang. This policy stands for every tick of this session; later ticks send only their trigger and snapshot. Trust disk over memory. ORDER: (1) Maintainer input first: poll docs/.annotations/inbox.json AND notes.json -- apply entries older than 5 minutes, clear at capture; records whose page is a docs/.grill/*.round.yaml are wizard submissions: apply IMMEDIATELY, delete the round file at capture. (2) If the trigger names an under-filled round buffer, compose the next wizard round BEFORE any landing (an empty maintainer inbox outranks lane plumbing): read pending rounds first and never re-ask them; keep two buffered; write docs/.grill/<topic>.round.yaml -- 3-5 ready decide rows batched by theme, every option carrying real verified code examples (delegate heavy example prep to a research worker) -- and ALWAYS verify the finished file both parses (python3 yaml.safe_load) AND serves clean (curl -s http://localhost:5173/__grill/round?topic=<topic>, expect 200) before the tick ends -- yaml.safe_load alone missed a round with valid YAML but no "question" string per question, which the mail UI rejected and which, until the isolation fix (kantord/toylang#164), blanked every OTHER pending round too, 2026-08-31. (3) Landing is DETERMINISTIC and automatic (serial queue, 2026-09-01): a worker exit fires .claude/scripts/land-lane.sh land <N> -- full just test gate in a throwaway worktree, straight onto main, pushed; a merge conflict or red gate re-dispatches the lane worker with the evidence in LAND-FAILURE.txt, cap 2, then leaves a land-failed marker. You NEVER fold, promote, read diffs pre-merge, or compose merge messages. Your landing duties instead: (a) a delegated row whose WORKTREE IS GONE has landed -- verify the Land commit in main log, board-archive.py the row; (b) a landable lane the event missed (trigger says so): run land-lane.sh land <N> DETACHED with nohup; (c) a land-failed marker in the trigger: if it says re-run land, do exactly that (detached); if the retry cap is spent, write one escalation question into a docs/.grill/ round (the lane, the LAND-FAILURE.txt evidence, options: rebrief with stronger model, reshape, drop) and rm the marker when acting on the ruling; (d) post-land review, AFTER other duties: read the newest Land commit diff on main and file follow-up board rows for real problems -- never edit main yourself. (4) Dispatch ready build rows into free lanes (cap 8; decide rows use no lane): .claude/scripts/dispatch-worker.sh <N> "<brief>" -- brief shape per the enwiro-delegate skill; record every rollout incident in plans/opencode-rollout.md. RULES: never edit a lane worktree file yourself -- a dead or half-done lane gets a continuation or research dispatch, however small the fix looks. A permission denial is a ruling, not an obstacle: NEVER re-attempt a blocked change through another channel (sed after a blocked Edit, a worker dispatched to make the same change, any workaround) -- write the proposed change as a question into a docs/.grill/ round for the maintainer and move on (maintainer rule, 2026-08-30). The docs dev server is the maintainers process: never start, stop, or restart it from a tick (a foreground restart wedged the tick lock 46 minutes) -- if it looks down, note that in the mail and move on. Never write an unbounded wait for a background task (lock, sentinel file, subagent): use a bounded primitive with an explicit give-up path -- the flock -w 1800 8 in land-lane.sh is the house pattern -- an ad hoc flock -x plus an infinite sentinel-file poll loop held a lock 90+ minutes and stalled every later tick, 2026-08-31. FAILURE STREAKS (snapshot carries runs= per lane): at 2-3 commitless runs read the last event log in ~/.cache/toylang-drive/opencode/ and rebrief with the actual root cause, never a repeat of a failed brief; at 4+ STOP -- no redispatch; write one escalation question into a docs/.grill/ round (the lane, the repeated root cause, options: stronger OPENCODE_MODEL, reshape, drop), touch ~/.cache/toylang-drive/escalated-issue-<N>, rm the marker when acting on the ruling. BOUND: one round composition plus one landing, or up to three landings (a cascade is one), then END the session even if more work is visible. Nothing changed: end quietly.'
fi

TS=$(date +%Y%m%d-%H%M%S)
OUT="$LOG_DIR/$TS-${1:-tick}-$MODEL.json"
echo "[drive-tick] $(date '+%H:%M:%S') ${1:-tick} starting on $MODEL -- $TRIGGER (log: $OUT)"
INBOX_N=$(python3 -c "
import json
d=json.load(open('docs/.annotations/inbox.json'))
print(len(d.get('records',[])))" 2>/dev/null || echo '?')
ROUNDS=$(ls docs/.grill/*.round.yaml 2>/dev/null | xargs -rn1 basename | tr '\n' ' ')
CORE="Trigger: $TRIGGER. Snapshot (from disk this second -- act on it, re-verify only what you modify):${STATE:- no delegated lanes} [inbox_records=$INBOX_N pending_rounds=${ROUNDS:-none}]. You are a ROUTER: turns are for decisions and the four scripts (dispatch-worker.sh, land-lane.sh, board-archive.py, round files), never exploration."

run_tick() { # $1: prompt
  # stream-json + the colorizer keeps the loop terminal a live, readable trace.
  local prompt=$1; shift
  # 9>&- : never leak the tick lock fd into the session or anything it spawns
  # (a tick-started dev server inherited it and held the lock 46 min, 2026-08-31).
  # timeout: a hung claude process (or a leaked background-task fd that never
  # delivers stdin EOF to tick-stream.py) must not hold the flock forever --
  # it held it 90+ min and stalled every later tick, 2026-08-31. Bounding this
  # one process guarantees fd 9 closes and the lock releases no matter what
  # inside the tick hangs; a killed tick just gets retried next interval.
  # the group, not just the first command, closes fd 9 for both pipeline
  # members -- a bare redirect on the claude command left tick-stream.py
  # (the pipe's second stage) still holding it (audit, 2026-08-31).
  { timeout --kill-after=30s 2700s \
      claude -p --model "$MODEL" --permission-mode auto \
      --output-format stream-json --verbose \
      "$prompt" 2>>"$LOG_DIR/errors.log" \
      | python3 "$REPO/.claude/scripts/tick-stream.py" "$OUT"; } 9>&-
}

run_tick "$POLICY $CORE"
