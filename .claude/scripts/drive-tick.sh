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
REPO=/home/kantord/repos/toylang
LANES="$HOME/.local/share/toylang-lanes"  # land-lane.sh's throwaway landing worktrees
LOG_DIR="$HOME/.cache/toylang-drive"
mkdir -p "$LOG_DIR"

# Never two ticks at once: a landing tick can outlive several loop intervals.
# $LOG_DIR, not /tmp: an unrelated host process holding an flock on a /tmp
# path via inode reuse has already stalled the sibling land.lock once
# (2026-09-06) -- nothing else touches $LOG_DIR.
exec 9>"$LOG_DIR/drive-tick.lock"
flock -n 9 || { echo "[drive-tick] $(date '+%H:%M:%S') another tick holds the lock (event-driven landing, most likely) -- yielded"; exit 0; }

export PATH="$HOME/.local/bin:$HOME/.local/share/pnpm:/usr/local/bin:/usr/bin:/bin"
cd "$REPO"

# Reclaim any msb sandbox left behind by an abruptly-killed dispatcher
# (reboot, OOM, kill -9) -- simple_dispatch.py's own teardown already
# handles the normal case (round-4 review of the design hardened this),
# this only catches the case where the whole process died before its own
# `finally` block ever ran. Mechanical, no model involved.
python3 "$REPO/.claude/scripts/dispatch-state.py" --gc >>"$LOG_DIR/sandbox-gc.log" 2>&1 || true

# The maintainer's mail UI depends on the dev server; revive it if a reboot ate it.
# A `( cmd & ) 9>&-` subshell does NOT reliably detach: bash's subshell-elision
# optimization can fork the backgrounded job directly off THIS shell (no
# intermediate subshell process at all), so the dev server ends up a literal
# child of drive-tick.sh, and the wrapper never exits again once the tick's own
# work is done -- confirmed live, 2026-09-09: a tick's real work (visible in its
# own log) finished cleanly but the bash process itself sat hung 5+ hours
# afterward with only the dev server as a descendant. The 2026-09-01 fix
# (setsid) only stopped it from pinning the tick LOCK; it never stopped the
# wrapper process itself from hanging. `bash -c '...' &` forks a genuine,
# independent child (standard fork+exec, no elision to worry about);
# `disown` drops it from this shell's job table so nothing can ever wait on
# it; `9>&-` on that command still closes the lock fd in the child before
# exec, verified via /proc/<pid>/fd that it does not leak through setsid's
# in-place exec chain.
if ! curl -s -o /dev/null --max-time 3 http://localhost:5173/toylang/dev/; then
  bash -c 'cd "$1" && exec setsid nohup pnpm dev --port 5173 --strictPort </dev/null >>"$2" 2>&1' \
    _ "$REPO/site" "$LOG_DIR/devserver.log" 9>&- &
  disown
fi

# Decide in bash whether this tick needs a model at all, and which one. A tick
# runs only for a reason; quiet healthy grinding (workers editing, nothing
# landable, no input) skips at zero token cost. Stuck-ness and orphaned commits
# are TRIGGERS, not things a skip can starve: a dead or 30-minute-silent lane
# and a non-delegated worktree sitting ahead of main both force a run.
MODEL=sonnet
TRIGGER=""
STATE=""
# Delegated-row state, read directly from simple_dispatch.py's own plain
# surfaces (plans/dispatch-log.csv + ~/.cache/toylang-simple-dispatch/results/)
# via dispatch-state.py -- not reconstructed from a worktree, a pgrep match,
# an ESCALATION.md file, or an opencode event log the way the old
# sandbox_dispatch.py/opencode pipeline required. simple_dispatch.py creates
# no persistent worktree at all (it clones into a disposable temp dir torn
# down inside the sandbox run itself), so "no worktree" is not a signal here
# the way it was under the old model -- there is never a worktree to find in
# the first place.
#
# DEAD_PRIORITY/DEAD_TRIGGER: kept only for the land-failed-marker loop
# below (still real and dispatch-mechanism-agnostic -- land-lane.sh's own
# retry/cap logic, unrelated to which script produced the branch). The old
# multi-tier lane-staleness ranking these once fed is gone: there is no
# partial "gone quiet" state to detect anymore under simple_dispatch.py's
# model (a row is either still dispatching, or has a definite terminal
# status in dispatch-log.csv) -- so nothing here raises above the land-failed
# tier (6) any longer.
DEAD_PRIORITY=-1
DEAD_TRIGGER=""
DELEGATED=$(python3 -c "
import yaml
for r in yaml.safe_load(open('plans/board.yaml')):
    if r.get('status') == 'delegated':
        print(r['id'])" 2>/dev/null | tr '\n' ' ')
LIVE_ROWS=$(python3 "$REPO/.claude/scripts/dispatch-state.py" --live 2>/dev/null)
for row_id in $DELEGATED; do
  if printf '%s\n' "$LIVE_ROWS" | grep -qxF "$row_id"; then
    STATE="$STATE [$row_id: dispatch still running]"
    continue  # still building -- not landed, not stuck
  fi
  RESULT=$(python3 "$REPO/.claude/scripts/dispatch-state.py" --status "$row_id" 2>/dev/null)
  if [ -z "$RESULT" ]; then
    # Delegated, no live dispatch process, and no dispatch-log.csv row at
    # all for this row -- the dispatcher itself was killed abruptly
    # (reboot, OOM, kill -9) before it ever reached its own finally block
    # (which always appends a row, even on CRASH). Identical in effect to
    # the old model's abrupt-kill case: not landed, reset to redispatch.
    TRIGGER="${TRIGGER:+$TRIGGER; }row $row_id was delegated but no dispatch ever completed (dispatcher likely killed abruptly) -- reset status: todo so it redispatches fresh"
    continue
  fi
  read -r RSTATUS RCOST RPATCH RREPORT <<<"$RESULT"
  STATE="$STATE [$row_id: $RSTATUS \$$RCOST]"
  case "$RSTATUS" in
    GREEN)
      TRIGGER="${TRIGGER:+$TRIGGER; }row $row_id is GREEN with a verified patch at $RPATCH -- land it: land-lane.sh land-patch $row_id $RPATCH"
      ;;
    STUCK|RED)
      # The model's own real-time explanation of what blocked it -- read
      # directly, no transcript reconstruction needed (see agent_loop.py's
      # self_report_blocker and plans/simple-dispatch-design.md's "Course
      # correction" section for why this replaced a separate LLM reviewer).
      SELF_REPORT=""
      [ -n "$RREPORT" ] && [ -f "$RREPORT" ] && SELF_REPORT=" -- agent's own report: $(cat "$RREPORT")"
      TRIGGER="${TRIGGER:+$TRIGGER; }row $row_id is $RSTATUS (cost \$$RCOST)$SELF_REPORT -- decide: narrower redispatch per the report, or a decide-row escalation"
      ;;
    TIMEOUT)
      TRIGGER="${TRIGGER:+$TRIGGER; }row $row_id timed out (cost \$$RCOST) -- likely an undersized budget, not unsolvable; consider one retry with a larger --overall-timeout before escalating"
      ;;
    SETUP_FAILED)
      TRIGGER="${TRIGGER:+$TRIGGER; }row $row_id failed to even start (host/sandbox setup issue, cost \$$RCOST) -- plausibly transient (network, sandbox boot); worth one plain retry"
      ;;
    FATAL)
      TRIGGER="${TRIGGER:+$TRIGGER; }row $row_id hit FATAL (bad key or no OpenRouter credit) -- fix the account before redispatching anything"
      ;;
  esac
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
# A free dispatcher with a ready row means dispatch is due. This JOINS the
# trigger instead of being a fallback: as a fallback it starved 2h behind the
# streak/starvation triggers while lanes sat idle (2026-08-30, under the old
# model -- the reasoning still applies). simple_dispatch.py's own
# ThreadPoolExecutor pool size IS the concurrency limit for one call, so
# "occupied" is now a simple binary (a live simple_dispatch.py process, or
# not) rather than a slot count -- dispatch-state.py --live is the single
# source of truth for this, read directly from real process cmdlines, not
# board.yaml's `status: delegated` (which can go stale exactly the way it
# already did under the old model).
DISPATCH=$(python3 "$REPO/.claude/scripts/dispatch-state.py" --dispatch-trigger 2>/dev/null)
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
  POLICY='Periodic audit (drive skill, "The periodic audit" section) for toylang at /home/kantord/repos/toylang. Reconstruct everything from disk; trust disk over anything remembered from earlier ticks. Check: every open GitHub issue maps to a board row; every delegated row has either a live simple_dispatch.py process (dispatch-state.py --live) or a real dispatch-log.csv row explaining its status; no GREEN row sits unlanded; plans/dispatch-log.csv and the real msb sandbox list (msb list) agree with each other, no orphans. Fix what is mechanical, file issues for the rest. End quietly if clean.'
else
  POLICY='Drive tick (drive skill, monitoring phase) for toylang at /home/kantord/repos/toylang. This policy stands for every tick of this session; later ticks send only their trigger and snapshot. Trust disk over memory. simple_dispatch.py + agent_loop.py is the ONLY dispatch mechanism (2026-09-11 ruling) -- no opencode, no lanes, no worktree-per-row; a dispatch clones into a disposable temp dir inside a disposable msb sandbox and reports through plans/dispatch-log.csv plus files under ~/.cache/toylang-simple-dispatch/results/, nothing else. ORDER: (1) Maintainer input first: poll docs/.annotations/inbox.json AND notes.json -- apply entries older than 5 minutes, clear at capture; records whose page is a docs/.grill/*.round.yaml are wizard submissions: apply IMMEDIATELY, delete the round file at capture. (2) If the trigger names an under-filled round buffer, compose the next wizard round BEFORE any landing (an empty maintainer inbox outranks dispatch plumbing): read pending rounds first and never re-ask them; keep two buffered; write docs/.grill/<topic>.round.yaml -- 3-5 ready decide rows batched by theme, every option carrying real verified code examples (delegate heavy example prep to a research worker) -- and ALWAYS verify the finished file both parses (python3 yaml.safe_load) AND serves clean (curl -s http://localhost:5173/__grill/round?topic=<topic>, expect 200) before the tick ends -- yaml.safe_load alone missed a round with valid YAML but no "question" string per question, which the mail UI rejected and which, until the isolation fix (kantord/toylang#164), blanked every OTHER pending round too, 2026-08-31. (3) Landing: a GREEN row in the trigger names its own verified patch path -- run .claude/scripts/land-lane.sh land-patch ROW-ID PATCH-PATH DETACHED with nohup (materializes a throwaway worktree from the patch, then the existing serial queue: full just test gate, straight onto main, pushed on green; a merge conflict or red gate re-dispatches automatically through simple_dispatch.py with the evidence in the brief, cap 2, then leaves a land-failed marker). You NEVER fold, promote, read diffs pre-merge, or compose merge messages. Your landing duties: (a) act on every GREEN row the trigger names, immediately; (b) a land-failed marker in the trigger: if it says re-run land, do exactly that (detached, same land-patch form -- the patch file is untouched by a failed land attempt); if the retry cap is spent, write one escalation question into a docs/.grill/ round (the row, the gate evidence, options: rebrief narrower per the agents own self-report, reshape, drop) and rm the marker when acting on the ruling; (c) post-land review, AFTER other duties: read the newest Land commit diff on main and file follow-up board rows for real problems -- never edit main yourself. (4) Dispatch: the trigger names ready build rows whenever dispatch-state.py --live is empty (the dispatcher is a single global batch, not a per-row slot pool -- never launch a second batch while one is already running). Write a brief for each ready row as plans/simple-briefs/ROW-ID.txt (enwiro-delegate skill content, this exact filename -- simple_dispatch.py requires it), then launch ONE call covering every ready row at once (up to 3), DETACHED -- nohup python3 .claude/scripts/simple_dispatch.py ROW-ID-1 ROW-ID-2 ROW-ID-3 --brief-dir plans/simple-briefs --parallel 3 >>~/.cache/toylang-drive/simple-dispatch.log 2>&1 & -- not one nohup per row: its own internal ThreadPoolExecutor IS the concurrency, and the whole process exits only once every row in the batch has a final status. It runs the full cycle unsupervised (real edits, its own just check verify with retries, patch extraction) and takes roughly 5-20 minutes, so never wait on it inline; set every row you dispatch to status: delegated in the same commit as writing its brief. A non-GREEN outcome (STUCK, RED, TIMEOUT, SETUP_FAILED, FATAL) already carries the agents own real explanation of what blocked it, verbatim, in the trigger text -- read that directly and act on the per-status guidance already there; there is no event log or ESCALATION.md to reconstruct anymore. FATAL means the account itself needs fixing -- flag it plainly, do not redispatch anything until it is. Record a real, surprising incident (a wrong self-report, a repeated failure shape, a cost anomaly) as a note in plans/simple-dispatch-design.md, not a new file. RULES: never edit a repo file yourself to fix a build row -- reshape the brief and redispatch, however small the fix looks (a dispatch is stateless per attempt and has nothing to build on from a hand-edit). A permission denial is a ruling, not an obstacle: NEVER re-attempt a blocked change through another channel (sed after a blocked Edit, a redispatch to make the same change, any workaround) -- write the proposed change as a question into a docs/.grill/ round for the maintainer and move on (maintainer rule, 2026-08-30). The docs dev server is the maintainers process: never start, stop, or restart it from a tick (a foreground restart wedged the tick lock 46 minutes) -- if it looks down, note that in the mail and move on. Never write an unbounded wait for a background task (lock, sentinel file, subagent): use a bounded primitive with an explicit give-up path -- the flock -w 1800 8 in land-lane.sh is the house pattern -- an ad hoc flock -x plus an infinite sentinel-file poll loop held a lock 90+ minutes and stalled every later tick, 2026-08-31. BOUND: one round composition plus one landing, or up to three landings (a cascade is one), or one dispatch batch, then END the session even if more work is visible. Nothing changed: end quietly.'
fi

TS=$(date +%Y%m%d-%H%M%S)
OUT="$LOG_DIR/$TS-${1:-tick}-$MODEL.json"
echo "[drive-tick] $(date '+%H:%M:%S') ${1:-tick} starting on $MODEL -- $TRIGGER (log: $OUT)"
INBOX_N=$(python3 -c "
import json
d=json.load(open('docs/.annotations/inbox.json'))
print(len(d.get('records',[])))" 2>/dev/null || echo '?')
ROUNDS=$(ls docs/.grill/*.round.yaml 2>/dev/null | xargs -rn1 basename | tr '\n' ' ')
CORE="Trigger: $TRIGGER. Snapshot (from disk this second -- act on it, re-verify only what you modify):${STATE:- no delegated rows} [inbox_records=$INBOX_N pending_rounds=${ROUNDS:-none}]. You are a ROUTER: turns are for decisions and the four scripts (simple_dispatch.py, land-lane.sh, board-archive.py, round files), never exploration. Nothing else dispatches build work -- sandbox_dispatch.py, dispatch-worker.sh, and opencode are retired."

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

# Coordinator-health check (2026-09-06): the tick's own claude -p call failing
# for a basic auth/API reason looks, in the log, just like a normal quiet
# tick -- nothing before this distinguished "nothing to do" from "the whole
# loop has been silently dead for the last N ticks" (a real OAuth expiry once
# went undetected for ~40 minutes). Track consecutive auth failures across
# ticks (this script is stateless per-run, so the streak lives in a file) and
# leave a hard-to-miss sentinel once the streak crosses a threshold, the same
# escalate-after-N-failures shape lane retries already use.
AUTH_STREAK_FILE="$LOG_DIR/coordinator-auth-fail-streak"
if grep -q "Failed to authenticate" "$OUT" 2>/dev/null; then
  STREAK=$(( $(cat "$AUTH_STREAK_FILE" 2>/dev/null || echo 0) + 1 ))
  echo "$STREAK" >"$AUTH_STREAK_FILE"
  if [ "$STREAK" -ge 3 ]; then
    # Notify once per outage, not once per tick: a passive sentinel file
    # only helps someone who happens to go looking for it, which defeats
    # the point for an unattended stretch (this exact gap went unnoticed
    # for ~40 minutes, 2026-09-06). The file's own presence is the
    # dedup -- only fire the notification on the tick that creates it.
    FIRST_DETECTION=0
    [ -f "$LOG_DIR/COORDINATOR-DOWN" ] || FIRST_DETECTION=1
    echo "$(date -Iseconds): $STREAK consecutive coordinator auth failures -- run \`claude /login\`" \
      >"$LOG_DIR/COORDINATOR-DOWN"
    echo "[drive-tick] $(date '+%H:%M:%S') $STREAK consecutive auth failures -- wrote $LOG_DIR/COORDINATOR-DOWN"
    if [ "$FIRST_DETECTION" -eq 1 ]; then
      DISPLAY="${DISPLAY:-:0}" notify-send "toylang coordinator down" \
        "$STREAK consecutive auth failures -- run 'claude /login'" 2>/dev/null || true
    fi
  fi
else
  rm -f "$AUTH_STREAK_FILE" "$LOG_DIR/COORDINATOR-DOWN"
fi
