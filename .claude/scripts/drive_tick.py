#!/usr/bin/env python3
"""Stateless drive tick: every tick is a brand-new claude -p session, no --resume.
Ticks are drilled to trust disk over memory, so a fresh session never loses state
-- resuming across ticks was a prompt-cache optimization only (measured saving:
~1-1.5k tokens/tick, well under a cent/tick), and the two flakiest ticks of
2026-08-31 (a 90+ minute lock stall, a 0-turn/25ms empty result) both happened on
a resumed session; every fresh-session tick that night did clean, verifiable work.
Dropped for reliability -- see plans/board.yaml and the drive skill for the ruling.

Runs in auto permission mode -- the same classifier guardrail interactive sessions
get. Every tick runs sonnet (maintainer rule, 2026-08-30): landing is mostly
plumbing, and review panels/subagents are retired outright (same-day ruling) --
the coordinator reads diffs itself. `audit` as argv[1] runs the audit prompt.
"""
import fcntl
import json
import os
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

import yaml

import dispatch_state
import tick_stream

REPO = Path("/home/kantord/repos/toylang")
LANES = Path.home() / ".local" / "share" / "toylang-lanes"  # land_lane.py's throwaway landing worktrees
LOG_DIR = Path.home() / ".cache" / "toylang-drive"
MEMORY = REPO / "plans" / "coordinator-memory.yaml"  # capped fact pool, plans/coordinator-memory-design.md
SCRIPTS = Path(__file__).resolve().parent
MODEL = "sonnet"


def log(msg: str) -> None:
    """Every operational status line (tick start/skip/lock-yield, signal
    failures, auth-streak state) used to go ONLY to stdout -- fine for `just
    peek` or a terminal someone happens to be watching, but confirmed as a
    real gap, 2026-09-11: if the loop runs unattended with nothing capturing
    its stdout (a systemd unit, a closed terminal, output accidentally sent
    to /dev/null), there is NO durable record that the loop is even alive,
    what a given tick decided, or when it last ran -- a skipped ("nothing to
    do") tick left zero trace anywhere, since the per-tick JSON result file
    is only written for ticks that actually invoke claude -p. Appends to a
    persistent file in addition to printing, so `just drive`'s own operator
    can always answer "is this still running, and what did it last do" from
    disk alone, independent of whatever happens to be attached to stdout."""
    print(msg)
    try:
        with open(LOG_DIR / "drive-tick.log", "a") as f:
            f.write(msg + "\n")
    except OSError:
        pass  # best-effort: the console print above already happened


def join_trigger(trigger: str, addition: str) -> str:
    return f"{trigger}; {addition}" if trigger else addition


def _revive_if_down(check_url: str, cmd: list[str], log_name: str) -> None:
    try:
        subprocess.run(["curl", "-s", "-o", os.devnull, "--max-time", "3", check_url], check=True)
        return
    except (subprocess.CalledProcessError, OSError):
        pass
    with open(LOG_DIR / log_name, "a") as devlog:  # not `log` -- shadows the module-level log() helper
        subprocess.Popen(
            cmd, cwd=REPO / "site", stdin=subprocess.DEVNULL, stdout=devlog,
            stderr=subprocess.STDOUT, start_new_session=True,
        )


def revive_dev_server() -> None:
    """The maintainer's tools app (Grill + Board + Annotations, on its own
    port since the grill-forest split, 2026-09-11) depends on these dev servers; revive
    whichever one a reboot ate. `subprocess.Popen(..., start_new_session=True)`
    gives a genuine, independent child via a real fork+exec -- no bash
    subshell-elision to worry about (bash's `(cmd &) 9>&-` optimization can
    fork the backgrounded job directly off the CURRENT shell with no
    intermediate subshell process at all, which is what made the dev server
    end up a literal child of drive-tick.sh and hung the wrapper 5+ hours,
    2026-09-09 -- Popen's own child process is never the calling process, so
    this failure mode does not exist here). Two independent processes, two
    independent checks: the tools app is deliberately its own Vite config/port
    (site/vite.tools.config.ts), not something `pnpm dev` on 5173 also serves
    anymore, so reviving one says nothing about whether the other is up."""
    _revive_if_down(
        "http://localhost:5173/toylang/dev/",
        ["pnpm", "dev", "--port", "5173", "--strictPort"],
        "devserver.log",
    )
    _revive_if_down(
        "http://localhost:5180/dev/",
        ["pnpm", "dev:tools"],
        "toolsserver.log",
    )


def maintainer_input_pending() -> bool:
    for fname in ("docs/.annotations/inbox.json", "docs/.annotations/notes.json"):
        path = REPO / fname
        try:
            d = json.loads(path.read_text())
        except (OSError, json.JSONDecodeError):
            continue
        if d.get("records") or d.get("composed"):
            return True
    # Outgoing docs/.grill/*.forest.yaml rounds WAIT on the maintainer -- only
    # submissions and annotation records count as input.
    return False


def open_forest_rounds() -> list[str]:
    """Forest rounds (docs/.grill/*.forest.yaml, the only grilling mechanism
    since 2026-09-16) that still have a `live` node waiting on the maintainer."""
    out = []
    for f in sorted((REPO / "docs" / ".grill").glob("*.forest.yaml")):
        try:
            d = yaml.safe_load(f.read_text())
        except (OSError, yaml.YAMLError):
            continue
        nodes = d.get("nodes", []) if isinstance(d, dict) else []
        if any(isinstance(n, dict) and n.get("status") == "live" for n in nodes):
            out.append(f.name)
    return out


def round_starvation_trigger() -> str | None:
    # Keep TWO rounds buffered (maintainer flow, 2026-08-30): grilling
    # happens WHILE workers grind, so finishing one round must always
    # reveal the next, not a wait.
    if len(open_forest_rounds()) >= 2:
        return None
    rows = yaml.safe_load(open(REPO / "plans" / "board.yaml"))
    live = {r["id"] for r in rows}
    ready = [r["id"] for r in rows
             if r.get("status") == "todo" and r.get("kind") == "decide"
             and all(n not in live for n in r.get("needs", []))]
    if not ready:
        return None
    return f"round buffer under-filled with {len(ready)} decide rows ready -- compose a forest round"


def exhaustion_trigger() -> str | None:
    rows = yaml.safe_load(open(REPO / "plans" / "board.yaml"))
    live = {r["id"] for r in rows}
    lanes = sum(1 for r in rows if r.get("status") == "delegated")
    ready = sum(1 for r in rows if r.get("status") == "todo" and r.get("kind") == "build"
                and all(n not in live for n in r.get("needs", [])))
    if lanes == 0 and ready == 0:
        return "board exhausted -- idle exception: self-originate 1-2 exploration rows (drive skill)"
    return None


def safe_signal(label: str, fn, *args, default=None):
    """Isolates one signal computation from the others. Under bash, each of
    these ran as its own `python3 -c ...` subprocess (most piped through
    `2>/dev/null`), so a crash in one -- a malformed board.yaml row, a
    dispatch-log.csv schema drift -- only zeroed out THAT check; every other
    signal, and the tick itself, kept working. A single shared Python
    process has no such isolation for free: an uncaught exception here would
    otherwise crash compute_trigger_and_state() (and the whole stateless,
    re-run-every-tick process) on every subsequent tick identically, with no
    escalation -- silently halting the autonomous coordinator. This restores
    the bash version's isolation explicitly."""
    try:
        return fn(*args)
    except Exception as e:
        log(f"[drive-tick] signal '{label}' failed (non-fatal, treated as "
            f"no signal): {e}")
        return default


def _already_landed(row_id: str) -> bool:
    r = subprocess.run(["git", "log", "main", "--format=%h", "--grep", f"^Land issue-{row_id}:"],
                       cwd=REPO, capture_output=True, text=True)
    return bool(r.stdout.strip())


def _process_delegated_row(row_id: str, live_rows: set[str]) -> tuple[str, list[str]]:
    """One delegated row's contribution to trigger/state. Kept as its own
    function -- and called through safe_signal() per row, not once for the
    whole loop -- because bash ran EACH row's status lookup as its own
    subprocess: a schema-drift crash on one row_id (e.g. a dispatch-log.csv
    row missing a column) produced empty output for that row only, while
    every other row_id's independent subprocess still succeeded. Wrapping
    the whole loop in one safe_signal (round-1 fix) only isolated the
    delegated-row SIGNAL as a unit -- one bad row would still have silently
    erased every OTHER healthy row's trigger (e.g. a real GREEN-row landing
    instruction) for that tick. This restores bash's actual per-row
    granularity."""
    if row_id in live_rows:
        return "", [f"[{row_id}: dispatch still running]"]  # still building -- not landed, not stuck
    row = dispatch_state.latest_row(row_id)
    if row is None:
        # Delegated, no live dispatch process, and no dispatch-log.csv row
        # at all for this row -- the dispatcher itself was killed abruptly
        # (reboot, OOM, kill -9) before it ever reached its own finally
        # block (which always appends a row, even on CRASH).
        return (f"row {row_id} was delegated but no dispatch ever completed "
                "(dispatcher likely killed abruptly) -- reset status: todo "
                "so it redispatches fresh"), []
    status, cost = row["status"], row["cost_usd"]
    report_path = dispatch_state.self_report_path_for(row_id, row["run_id"])
    state = [f"[{row_id}: {status} ${cost}]"]
    if status == "GREEN":
        if row_id in dispatch_state.live_land_row_ids():
            return "", [f"[{row_id}: GREEN, landing in progress]"]
        if _already_landed(row_id):
            return (f"row {row_id} is GREEN and its Land commit is already on main -- archive it "
                    f"(board-archive.py {row_id}), do NOT land again"), state
        return (f"row {row_id} is GREEN with a verified patch at "
                f"{row.get('patch_path', '')} -- land it: uv run --project "
                f".claude/scripts .claude/scripts/land_lane.py land-patch "
                f"{row_id} {row.get('patch_path', '')}"), state
    if status in ("STUCK", "RED"):
        # The model's own real-time explanation of what blocked it -- read
        # directly, no transcript reconstruction needed (see agent_loop.py's
        # self_report_blocker and plans/simple-dispatch-design.md's "Course
        # correction" section for why this replaced a separate LLM reviewer).
        self_report = ""
        if report_path and Path(report_path).is_file():
            self_report = f" -- agent's own report: {Path(report_path).read_text(errors='replace')}"
        # Who ended the run is stated before the report, and the decision
        # has three verbs, not two. Before 2026-09-14 a self-report saying
        # "the harness cut me off, not scope" could only be routed as
        # "redispatch narrower" or "escalate the row" -- 12 rows went to
        # the maintainer that way for one harness bug.
        ended_by = row.get("ended_by") or "unrecorded"
        shape = (f"ended_by={ended_by} edits={row.get('edits') or '?'} "
                 f"turns={row.get('turns') or '?'}")
        harness = ended_by in dispatch_state.HARNESS_ENDINGS and (row.get("edits") or "0") == "0"
        verdict = (" -- a harness ending with zero edits is the HARNESS's decision, not the "
                   "model's: treat as a pipeline defect (see the dispatch health line), "
                   "not a scope problem" if harness else "")
        return (f"row {row_id} is {status} ({shape}, cost ${cost}){self_report}{verdict} -- "
                f"read `uv run --project .claude/scripts .claude/scripts/dispatch_state.py --show "
                f"{row_id}` first, then decide: (a) redispatch narrower per the report, (b) a "
                "decide-row escalation if the report says scope, or (c) harness defect: hold "
                "dispatch, --capture the run, one harness decide row"), state
    if status == "TIMEOUT":
        return (f"row {row_id} timed out (cost ${cost}) -- likely an undersized "
                "budget, not unsolvable; consider one retry with a larger "
                "--overall-timeout before escalating"), state
    if status == "SETUP_FAILED":
        return (f"row {row_id} failed to even start (host/sandbox setup issue, "
                f"cost ${cost}) -- plausibly transient (network, sandbox boot); "
                "worth one plain retry"), state
    if status == "FATAL":
        return (f"row {row_id} hit FATAL (bad key or no OpenRouter credit) -- "
                "fix the account before redispatching anything"), state
    return "", state


def _delegated_row_signal() -> tuple[str, list[str]]:
    # Delegated-row state, read directly from simple_dispatch.py's own plain
    # surfaces (plans/dispatch-log.csv + ~/.cache/toylang-simple-dispatch/results/)
    # via dispatch_state -- not reconstructed from a worktree, a pgrep match,
    # an ESCALATION.md file, or an opencode event log the way the old
    # sandbox_dispatch.py/opencode pipeline required. simple_dispatch.py
    # creates no persistent worktree at all (it clones into a disposable
    # temp dir torn down inside the sandbox run itself), so "no worktree" is
    # not a signal here the way it was under the old model -- there is
    # never a worktree to find in the first place.
    trigger = ""
    state_parts: list[str] = []
    board_rows = yaml.safe_load(open(REPO / "plans" / "board.yaml"))
    delegated = [r["id"] for r in board_rows if r.get("status") == "delegated"]
    live_rows = set(dispatch_state.live_row_ids())
    for row_id in delegated:
        row_trigger, row_state = safe_signal(
            f"delegated row {row_id}", _process_delegated_row, row_id, live_rows,
            default=("", [f"[{row_id}: status lookup failed, see stderr]"]))
        if row_trigger:
            trigger = join_trigger(trigger, row_trigger)
        state_parts += row_state
    return trigger, state_parts


def _land_failed_signal() -> tuple[str, list[str]]:
    # Landing failures (serial queue, 2026-09-01): land_lane.py handles its
    # own conflict/red re-dispatches (cap 2); a marker here means the cap is
    # spent (or the main checkout stayed busy) and the tick must route it.
    # Tier 6 so a blocked landing of finished work always outranks routine
    # lane chatter (the lesson of the accumulator era: promotion triggers
    # starved behind dead-lane rebriefs all night, 2026-08-31).
    dead_priority = -1
    dead_trigger = ""
    state_parts: list[str] = []
    for marker in sorted(LOG_DIR.glob("land-failed-issue-*")):
        n = marker.name.removeprefix("land-failed-issue-")
        content = marker.read_text(errors="replace").strip()
        state_parts.append(f"[land-failed: issue-{n} -- {content}]")
        if 6 > dead_priority:
            dead_priority = 6
            dead_trigger = f"landing of issue-{n} is stuck ({content}) -- route it"
    return dead_trigger, state_parts


def _memory_slot_state(slot: dict) -> str:
    # A watch is rendered as the condition itself, not a fact: the 2026-09-11
    # "worth watching" note about the no-progress cutoff was correct and sat
    # unread in plans/simple-dispatch-design.md while the shape recurred in
    # 46 of the next 54 runs. Putting it in the snapshot is what makes a
    # watch different from a note -- every tick re-reads it whether or not
    # it meant to.
    if slot["kind"] == "watch":
        return f"[watch: {slot['condition']} -- {slot['observation']}]"
    return f"[mem: {slot['kind']} {slot['id']}: {slot['summary']} ({slot['source']})]"


def _memory_signal() -> list[str]:
    slots = yaml.safe_load(open(MEMORY))["slots"]
    # Per-slot isolation: one malformed slot logs and drops out, the rest
    # still reach the snapshot (the same shape as _delegated_row_signal).
    rendered = [safe_signal(f"memory slot {s.get('id', '?')}", _memory_slot_state, s)
                for s in slots]
    return [line for line in rendered if line]


def _health_signal() -> tuple[str, str]:
    h = dispatch_state.health()
    line = dispatch_state.format_health(h)
    alarm = dispatch_state.health_trigger() or ""
    if alarm:
        # Mechanical, so the evidence exists in git before any tick decides
        # anything: capture the latest run of every zero-edit row named by
        # the alarm. Idempotent per day (same destination directory).
        captured = []
        for row_id in dict.fromkeys(h["zero_edit_rows"]):
            dest = safe_signal(f"capture {row_id}", dispatch_state.capture_incident, row_id, default=None)
            if dest:
                captured.append(str(dest.relative_to(REPO)))
        if captured:
            alarm += " -- evidence captured: " + " ".join(captured) + " (commit these)"
    return line, alarm


def compute_trigger_and_state(mode: str) -> tuple[str, str]:
    trigger = ""
    state_parts: list[str] = []

    delegated_trigger, delegated_state = safe_signal(
        "delegated-row state", _delegated_row_signal, default=("", []))
    if delegated_trigger:
        trigger = join_trigger(trigger, delegated_trigger)
    state_parts += delegated_state

    dead_trigger, land_failed_state = safe_signal(
        "land-failed markers", _land_failed_signal, default=("", []))
    state_parts += land_failed_state
    if not trigger and dead_trigger:
        trigger = dead_trigger

    state_parts += safe_signal("coordinator memory", _memory_signal, default=[])

    # Population view of recent runs. ALWAYS in the state snapshot so a tick
    # sees the shape of the last 20 runs, and JOINS the trigger when the
    # alarm is on -- the 2026-09-14 incident was 46 runs with the same
    # harness ending, each judged alone as a task problem.
    health_line, health_alarm = safe_signal("dispatch health", _health_signal, default=("", ""))
    if health_line:
        state_parts.append(f"[{health_line}]")
    if health_alarm:
        trigger = join_trigger(trigger, health_alarm)

    # Maintainer input always runs the tick (the 5-minute quiet rule is
    # judged inside).
    if safe_signal("maintainer input", maintainer_input_pending, default=False):
        trigger = trigger or "maintainer input pending"

    # Decide starvation: the maintainer keeps checking an empty inbox while
    # decide rows sit ready. NOT a fallback (it starved twice, 2026-08-30:
    # as a fallback it lost to every landing and dead-lane trigger, and the
    # maintainer drained both buffered rounds in ten minutes with nothing
    # refilling) -- an under-filled round buffer ALWAYS joins the trigger,
    # alongside whatever else the tick has.
    starve = safe_signal("round starvation", round_starvation_trigger)
    if starve:
        trigger = join_trigger(trigger, starve)

    # A free dispatcher with a ready row means dispatch is due. This JOINS
    # the trigger instead of being a fallback: as a fallback it starved 2h
    # behind the streak/starvation triggers while lanes sat idle
    # (2026-08-30, under the old model -- the reasoning still applies).
    # simple_dispatch.py's own ThreadPoolExecutor pool size IS the
    # concurrency limit for one call, so "occupied" is now a simple binary
    # (a live simple_dispatch.py process, or not) rather than a slot count.
    dispatch = safe_signal("dispatch trigger", dispatch_state.dispatch_trigger,
                            dispatch_state.DEFAULT_CAP)
    if dispatch:
        trigger = join_trigger(trigger, dispatch)

    # Exhaustion: nothing delegated, nothing ready to build -- the idle
    # exception (drive skill) lets the tick self-originate one or two
    # exploration rows.
    if not trigger:
        exhausted = safe_signal("exhaustion", exhaustion_trigger)
        if exhausted:
            trigger = exhausted

    if mode == "audit":
        trigger = "scheduled audit"

    state = " ".join(state_parts)
    return trigger, state


AUDIT_POLICY = (
    'Periodic audit (drive skill, "The periodic audit" section) for toylang at '
    '/home/kantord/repos/toylang. Reconstruct everything from disk; trust disk '
    'over anything remembered from earlier ticks. Check: every open GitHub '
    'issue maps to a board row; every delegated row has either a live '
    'simple_dispatch.py process (uv run --project .claude/scripts '
    '.claude/scripts/dispatch_state.py --live) or a real dispatch-log.csv row '
    'explaining its status; no GREEN row sits unlanded; plans/dispatch-log.csv '
    'and the real msb sandbox list (msb list) agree with each other, no '
    'orphans. Coordinator memory (plans/coordinator-memory.yaml): for every '
    'slot, re-resolve its source and check the fact still holds -- bump '
    'confirmed when it does, drop the slot when it does not (a watch is '
    'dropped once its condition has been acted on or has stopped recurring); '
    'a slot nobody ever confirms is the kill signal for the whole pool. Fix '
    'what is mechanical, file issues for the rest. End quietly if clean.'
)

TICK_POLICY = (
    'Drive tick (drive skill, monitoring phase) for toylang at '
    '/home/kantord/repos/toylang. This policy stands for every tick of this '
    'session; later ticks send only their trigger and snapshot. Trust disk '
    'over memory. simple_dispatch.py + agent_loop.py is the ONLY dispatch '
    'mechanism (2026-09-11 ruling) -- no opencode, no lanes, no worktree-per-row; '
    'a dispatch clones into a disposable temp dir inside a disposable msb '
    'sandbox and reports through plans/dispatch-log.csv plus files under '
    '~/.cache/toylang-simple-dispatch/results/, nothing else. ORDER: '
    '(1) Maintainer input first: poll docs/.annotations/inbox.json AND '
    'notes.json -- apply entries older than 5 minutes, clear at capture; '
    'records whose page is a docs/.grill/*.forest.yaml are forest-round '
    'submissions: apply IMMEDIATELY -- write the answer INTO the node first '
    '(status: answered + answer block), clear the inbox record second. (2) If '
    'the trigger names an under-filled round buffer, compose the next forest '
    'round BEFORE any landing (an empty maintainer inbox outranks dispatch '
    'plumbing): read pending rounds first and never re-ask them; keep two '
    'buffered; write docs/.grill/<topic>.forest.yaml -- ONE live root node per '
    'file (the validator rejects two live roots), one ready decide row per '
    'topic, every option carrying real verified code examples (delegate heavy '
    'example prep to a research worker) -- and ALWAYS verify the finished file '
    'both parses (python3 yaml.safe_load) AND serves clean (curl -s '
    'http://localhost:5180/__grill-forest/round?topic=<topic>, expect 200) '
    'before the tick ends -- a parseable file the validator rejects shows the '
    'maintainer an error card instead of a question, and until the isolation '
    'fix (kantord/toylang#164) one bad file blanked every OTHER '
    'pending round too, 2026-08-31. (3) Landing: a GREEN row in the trigger '
    'names its own verified patch path -- run uv run --project .claude/scripts '
    '.claude/scripts/land_lane.py land-patch ROW-ID PATCH-PATH DETACHED with '
    'nohup (materializes a throwaway worktree from the patch, then the '
    'existing serial queue: full just test gate, straight onto main, pushed '
    'on green; a merge conflict or red gate re-dispatches automatically '
    'through simple_dispatch.py with the evidence in the brief, cap 2, then '
    'leaves a land-failed marker). You NEVER fold, promote, read diffs '
    'pre-merge, or compose merge messages. Your landing duties: (a) act on '
    'every GREEN row the trigger names, immediately; (b) a land-failed marker '
    'in the trigger: if it says re-run land, do exactly that (detached, same '
    'land-patch form -- the patch file is untouched by a failed land '
    'attempt); if the retry cap is spent, write one escalation question into '
    'a docs/.grill/ round (the row, the gate evidence, options: rebrief '
    'narrower per the agents own self-report, reshape, drop) and rm the '
    'marker when acting on the ruling; (c) post-land review, AFTER other '
    'duties: read the newest Land commit diff on main and file follow-up '
    'board rows for real problems -- never edit main yourself. (4) Dispatch: '
    'the trigger names ready build rows whenever '
    'uv run --project .claude/scripts .claude/scripts/dispatch_state.py '
    '--live is empty (the dispatcher is a single global batch, not a per-row '
    'slot pool -- never launch a second batch while one is already running). '
    'Write a brief for each ready row as plans/simple-briefs/ROW-ID.txt '
    '(enwiro-delegate skill content, this exact filename -- simple_dispatch.py '
    'requires it), then launch ONE call covering every ready row at once (up '
    'to 3), DETACHED -- nohup uv run --project .claude/scripts '
    '.claude/scripts/simple_dispatch.py ROW-ID-1 ROW-ID-2 ROW-ID-3 --brief-dir '
    'plans/simple-briefs --parallel 3 >>~/.cache/toylang-drive/simple-dispatch.log '
    '2>&1 & -- not one nohup per row: its own internal ThreadPoolExecutor IS '
    'the concurrency, and the whole process exits only once every row in the '
    'batch has a final status. It runs the full cycle unsupervised (real '
    'edits, its own just check verify with retries, patch extraction) and '
    'takes roughly 5-20 minutes, so never wait on it inline; set every row you '
    'dispatch to status: delegated in the same commit as writing its brief. A '
    'non-GREEN outcome (STUCK, RED, TIMEOUT, SETUP_FAILED, FATAL) names in the '
    'trigger WHO ended it (ended_by: model_done, no_progress_cutoff, max_turns, '
    'dedup, wall_clock, api_error, setup, crash), how many edits and turns it '
    'made, and the agents own explanation; the standard way to look at any '
    'such run is uv run --project .claude/scripts .claude/scripts/dispatch_state.py '
    '--show ROW-ID -- run it before deciding, never grep the cache by hand. '
    'THREE verbs, not two: (a) redispatch narrower per the report; (b) a '
    'decide-row escalation when the report says scope; (c) HARNESS DEFECT when '
    'ended_by is a harness ending with zero edits, or when the dispatch health '
    'line in the snapshot says ALARM -- then hold dispatch, run dispatch_state.py '
    '--capture ROW-ID for the evidence (commit plans/incidents/), and open or '
    'update ONE harness decide row naming the common ended_by; never turn N '
    'STUCK rows into N round questions (more than 4 escalation questions in a '
    'round is a board-lint error). A run that cost little and changed nothing '
    'is a zero, not a cheap failure: count zero-edit runs, not dollars. FATAL '
    'means the account itself needs fixing -- flag it plainly, do not '
    'redispatch anything until it is. The snapshot carries [mem: ...] and '
    '[watch: ...] lines from plans/coordinator-memory.yaml: a mem line is a '
    'fact a past tick already verified (do not re-derive it; re-check only '
    'what you are about to modify); a watch line is a condition to hold every '
    'non-GREEN run and the dispatch health line against -- when one matches, '
    'name the watch in your reasoning and act as it says instead of judging '
    'the run alone. WRITE to that file only as a byproduct of real work, in '
    'the same commit: a footprint conflict or static fact you had to establish '
    'while doing something else (kind footprint-conflict, fact-check, other), '
    'or a hunch you would otherwise leave as "worth watching" in a note (kind '
    'watch: one-line condition plus the observation with date and pointer); '
    'never as a dedicated check-memory step, never over cap 8, and board-lint.py '
    'rejects any other kind. Record a real, surprising incident (a '
    'wrong self-report, a repeated failure shape, a cost anomaly) as a note in '
    'plans/simple-dispatch-design.md, not a new file. RULES: never edit a repo '
    'file yourself to fix a build row -- '
    'reshape the brief and redispatch, however small the fix looks (a '
    'dispatch is stateless per attempt and has nothing to build on from a '
    'hand-edit). A permission denial is a ruling, not an obstacle: NEVER '
    're-attempt a blocked change through another channel (sed after a '
    'blocked Edit, a redispatch to make the same change, any workaround) -- '
    'write the proposed change as a question into a docs/.grill/ round for '
    'the maintainer and move on (maintainer rule, 2026-08-30). The docs dev '
    'server is the maintainers process: never start, stop, or restart it '
    'from a tick (a foreground restart wedged the tick lock 46 minutes) -- if '
    'it looks down, note that in the mail and move on. Never write an '
    'unbounded wait for a background task (lock, sentinel file, subagent): '
    'use a bounded primitive with an explicit give-up path -- the bounded '
    'flock wait in land_lane.py is the house pattern -- an ad hoc flock -x '
    'plus an infinite sentinel-file poll loop held a lock 90+ minutes and '
    'stalled every later tick, 2026-08-31. BOUND: one round composition plus '
    'one landing, or up to three landings (a cascade is one), or one dispatch '
    'batch, then END the session even if more work is visible. Nothing '
    'changed: end quietly.'
)


def run_tick(prompt: str, out_path: Path) -> None:
    """`timeout --kill-after=30s 2700s` still wraps `claude -p` directly, as
    a literal subprocess -- a real, load-bearing hard-kill guarantee (this is
    literally how the 90-minute lock-stall incident of 2026-08-31 got
    bounded), NOT reimplemented as a Python deadline-and-kill loop. Reading
    proc.stdout in this for-loop drains it live, WHILE claude -p is still
    running -- unlike agent_loop.py's run_bash pattern (which polls without
    touching stdout, then reads once at the end -- wrong shape here, since
    tick_stream must render each line live for "keeps the loop terminal a
    live, readable trace"). Breaking on the terminal event (tick_stream's own
    job) still means not waiting on stdout EOF, which a leaked
    background-task fd can withhold forever (held the tick lock 90+ min,
    2026-08-31) -- a second, independent defense on top of the outer
    `timeout`, not a replacement for it."""
    errors_log = LOG_DIR / "errors.log"
    with open(errors_log, "a") as errf:
        proc = subprocess.Popen(
            ["timeout", "--kill-after=30s", "2700s",
             "claude", "-p", "--model", MODEL, "--permission-mode", "auto",
             "--output-format", "stream-json", "--verbose", prompt],
            stdout=subprocess.PIPE, stderr=errf, text=True,
        )
        assert proc.stdout is not None  # guaranteed by stdout=PIPE above; typeshed can't see that
        try:
            for line in proc.stdout:
                try:
                    if tick_stream.process_line(line, str(out_path)):
                        break
                except Exception as e:
                    # A malformed line must not crash the whole tick (and
                    # skip check_coordinator_auth() for it) -- this used to
                    # be isolated for free by tick-stream.py running as a
                    # separate subprocess under bash; rendering in-process
                    # now needs the same isolation explicitly.
                    log(f"[drive-tick] tick_stream.process_line failed on "
                        f"one line (non-fatal): {e}")
        finally:
            proc.stdout.close()
            try:
                proc.wait(timeout=35)
            except subprocess.TimeoutExpired:
                proc.kill()
                proc.wait()


def check_coordinator_auth(out_path: Path) -> None:
    """The tick's own claude -p call failing for a basic auth/API reason
    looks, in the log, just like a normal quiet tick -- nothing before this
    distinguished "nothing to do" from "the whole loop has been silently
    dead for the last N ticks" (a real OAuth expiry once went undetected for
    ~40 minutes). Track consecutive auth failures across ticks (this process
    is stateless per-run, so the streak lives in a file) and leave a
    hard-to-miss sentinel once the streak crosses a threshold."""
    streak_file = LOG_DIR / "coordinator-auth-fail-streak"
    down_file = LOG_DIR / "COORDINATOR-DOWN"
    auth_failed = False
    if out_path.exists():
        auth_failed = "Failed to authenticate" in out_path.read_text(errors="replace")
    if auth_failed:
        streak = int(streak_file.read_text().strip() or "0") + 1 if streak_file.exists() else 1
        streak_file.write_text(f"{streak}\n")
        if streak >= 3:
            # Notify once per outage, not once per tick: a passive sentinel
            # file only helps someone who happens to go looking for it,
            # which defeats the point for an unattended stretch (this exact
            # gap went unnoticed for ~40 minutes, 2026-09-06). The file's
            # own presence is the dedup -- only fire the notification on the
            # tick that creates it.
            first_detection = not down_file.exists()
            down_file.write_text(
                f"{datetime.now().astimezone().isoformat()}: {streak} consecutive "
                "coordinator auth failures -- run `claude /login`\n")
            log(f"[drive-tick] {datetime.now():%H:%M:%S} {streak} consecutive "
                f"auth failures -- wrote {down_file}")
            if first_detection:
                env = dict(os.environ)
                # bash's ${DISPLAY:-:0} treats an empty-but-set DISPLAY the
                # same as unset; env.setdefault would not (it only fires
                # when the key is absent), silently handing notify-send an
                # empty display it fails against with stderr discarded.
                env["DISPLAY"] = os.environ.get("DISPLAY") or ":0"
                subprocess.run(
                    ["notify-send", "toylang coordinator down",
                     f"{streak} consecutive auth failures -- run 'claude /login'"],
                    env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        streak_file.unlink(missing_ok=True)
        down_file.unlink(missing_ok=True)


def main() -> int:
    mode = sys.argv[1] if len(sys.argv) > 1 else "tick"
    LOG_DIR.mkdir(parents=True, exist_ok=True)

    # Never two ticks at once: a landing tick can outlive several loop
    # intervals. LOG_DIR, not /tmp: an unrelated host process holding an
    # flock on a /tmp path via inode reuse has already stalled the sibling
    # land.lock once (2026-09-06) -- nothing else touches LOG_DIR.
    lock_file = open(LOG_DIR / "drive-tick.lock", "w")
    try:
        fcntl.flock(lock_file, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        log(f"[drive-tick] {datetime.now():%H:%M:%S} another tick holds the "
            "lock (event-driven landing, most likely) -- yielded")
        return 0

    os.environ["PATH"] = (f"{Path.home()}/.local/bin:{Path.home()}/.local/share/pnpm:"
                           "/usr/local/bin:/usr/bin:/bin")
    os.chdir(REPO)

    # Reclaim any msb sandbox left behind by an abruptly-killed dispatcher
    # (reboot, OOM, kill -9) -- simple_dispatch.py's own teardown already
    # handles the normal case, this only catches the case where the whole
    # process died before its own `finally` block ever ran. Mechanical, no
    # model involved. Called directly, not via subprocess: dispatch_state is
    # already Python, in the same project.
    # Always writes ONE line, even when nothing was removed -- a real
    # observability gap, confirmed 2026-09-11: the old version only wrote on
    # an actual removal, so "GC ran and found nothing to clean" and "GC
    # silently stopped running entirely" were indistinguishable from the log
    # alone (both leave the file untouched). A per-tick heartbeat closes
    # that: the file's own mtime now proves execution regardless of outcome.
    sandbox_gc_log = LOG_DIR / "sandbox-gc.log"
    ts = datetime.now().strftime("%Y-%m-%d %H:%M:%S")
    try:
        removed = dispatch_state.gc_orphaned_sandboxes()
        # Bundle retention rides the same heartbeat: keep the last 3 runs
        # per row plus anything still delegated or named in a pending round.
        removed_bundles = dispatch_state.gc_bundles()
        with open(sandbox_gc_log, "a") as f:
            if removed or removed_bundles:
                for line in removed:
                    f.write(f"{ts} removed orphaned sandbox: {line}\n")
                for line in removed_bundles:
                    f.write(f"{ts} removed old bundle: {line}\n")
            else:
                f.write(f"{ts} gc ran, 0 orphaned sandboxes, 0 old bundles\n")
    except Exception as e:
        with open(sandbox_gc_log, "a") as f:
            f.write(f"{ts} gc failed (non-fatal): {e}\n")

    revive_dev_server()

    trigger, state = compute_trigger_and_state(mode)

    # Cheap, deterministic, every tick (not just the audit) -- board_revival_check.py flags a
    # parked row whose `needs` are now all done, purely on structured board data. It can't
    # judge whether that's the SAME thing as the row's `blocked_by` reason actually clearing
    # (see the script's own docstring), so it's folded into the trigger as a nudge for the tick
    # to read and verify, never an instruction to just flip status on the finding alone.
    try:
        revival = subprocess.run(
            [sys.executable, str(SCRIPTS / "board_revival_check.py")],
            cwd=REPO, capture_output=True, text=True, timeout=10,
        ).stdout.strip()
    except (subprocess.SubprocessError, OSError):
        revival = ""
    if revival:
        trigger = join_trigger(trigger, f"revival check: {revival}")

    if not trigger:
        log(f"[drive-tick] {datetime.now():%H:%M:%S} nothing to do (workers "
            "grinding, no input) -- skipped, zero tokens")
        return 0

    policy = AUDIT_POLICY if mode == "audit" else TICK_POLICY

    ts = time.strftime("%Y%m%d-%H%M%S")
    out_path = LOG_DIR / f"{ts}-{mode}-{MODEL}.json"
    log(f"[drive-tick] {datetime.now():%H:%M:%S} {mode} starting on {MODEL} "
        f"-- {trigger} (log: {out_path})")

    try:
        inbox_n = str(len(json.loads((REPO / "docs/.annotations/inbox.json").read_text())
                          .get("records", [])))
    except (OSError, json.JSONDecodeError):
        inbox_n = "?"
    rounds = " ".join(open_forest_rounds())
    core = (f"Trigger: {trigger}. Snapshot (from disk this second -- act on it, "
            f"re-verify only what you modify):{state or ' no delegated rows'} "
            f"[inbox_records={inbox_n} pending_rounds={rounds or 'none'}]. You are "
            "a ROUTER: turns are for decisions and the four scripts "
            "(simple_dispatch.py, land_lane.py, board-archive.py, forest files), "
            "never exploration. Nothing else dispatches build work -- "
            "sandbox_dispatch.py, dispatch-worker.sh, and opencode are retired.")

    run_tick(f"{policy} {core}", out_path)
    check_coordinator_auth(out_path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
