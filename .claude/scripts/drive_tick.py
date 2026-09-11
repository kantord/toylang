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

import dispatch_state
import tick_stream
import yaml

REPO = Path("/home/kantord/repos/toylang")
LANES = Path.home() / ".local" / "share" / "toylang-lanes"  # land_lane.py's throwaway landing worktrees
LOG_DIR = Path.home() / ".cache" / "toylang-drive"
SCRIPTS = Path(__file__).resolve().parent
MODEL = "sonnet"


def join_trigger(trigger: str, addition: str) -> str:
    return f"{trigger}; {addition}" if trigger else addition


def revive_dev_server() -> None:
    """The maintainer's mail UI depends on the dev server; revive it if a
    reboot ate it. `subprocess.Popen(..., start_new_session=True)` gives a
    genuine, independent child via a real fork+exec -- no bash
    subshell-elision to worry about (bash's `(cmd &) 9>&-` optimization can
    fork the backgrounded job directly off the CURRENT shell with no
    intermediate subshell process at all, which is what made the dev server
    end up a literal child of drive-tick.sh and hung the wrapper 5+ hours,
    2026-09-09 -- Popen's own child process is never the calling process, so
    this failure mode does not exist here)."""
    try:
        subprocess.run(["curl", "-s", "-o", os.devnull, "--max-time", "3",
                         "http://localhost:5173/toylang/dev/"], check=True)
        return
    except (subprocess.CalledProcessError, OSError):
        pass
    devserver_log = LOG_DIR / "devserver.log"
    with open(devserver_log, "a") as log:
        subprocess.Popen(
            ["pnpm", "dev", "--port", "5173", "--strictPort"],
            cwd=REPO / "site", stdin=subprocess.DEVNULL, stdout=log,
            stderr=subprocess.STDOUT, start_new_session=True,
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
    # Outgoing *.round.yaml files WAIT on the maintainer -- only submissions
    # and annotation records count as input.
    grill_dir = REPO / "docs" / ".grill"
    if grill_dir.is_dir():
        for f in grill_dir.iterdir():
            if not f.name.endswith(".round.yaml"):
                return True
    return False


def round_starvation_trigger() -> str | None:
    # Keep TWO rounds buffered (maintainer flow, 2026-08-30): grilling
    # happens WHILE workers grind, so finishing one round must always
    # reveal the next, not a wait.
    rounds = list((REPO / "docs" / ".grill").glob("*.round.yaml"))
    if len(rounds) >= 2:
        return None
    rows = yaml.safe_load(open(REPO / "plans" / "board.yaml"))
    live = {r["id"] for r in rows}
    ready = [r["id"] for r in rows
             if r.get("status") == "todo" and r.get("kind") == "decide"
             and all(n not in live for n in r.get("needs", []))]
    if not ready:
        return None
    return f"round buffer under-filled with {len(ready)} decide rows ready -- compose a grill round"


def exhaustion_trigger() -> str | None:
    rows = yaml.safe_load(open(REPO / "plans" / "board.yaml"))
    live = {r["id"] for r in rows}
    lanes = sum(1 for r in rows if r.get("status") == "delegated")
    ready = sum(1 for r in rows if r.get("status") == "todo" and r.get("kind") == "build"
                and all(n not in live for n in r.get("needs", [])))
    if lanes == 0 and ready == 0:
        return "board exhausted -- idle exception: self-originate 1-2 exploration rows (drive skill)"
    return None


def compute_trigger_and_state(mode: str) -> tuple[str, str]:
    trigger = ""
    state_parts: list[str] = []

    # Delegated-row state, read directly from simple_dispatch.py's own plain
    # surfaces (plans/dispatch-log.csv + ~/.cache/toylang-simple-dispatch/results/)
    # via dispatch_state -- not reconstructed from a worktree, a pgrep match,
    # an ESCALATION.md file, or an opencode event log the way the old
    # sandbox_dispatch.py/opencode pipeline required. simple_dispatch.py
    # creates no persistent worktree at all (it clones into a disposable
    # temp dir torn down inside the sandbox run itself), so "no worktree" is
    # not a signal here the way it was under the old model -- there is
    # never a worktree to find in the first place.
    board_rows = yaml.safe_load(open(REPO / "plans" / "board.yaml"))
    delegated = [r["id"] for r in board_rows if r.get("status") == "delegated"]
    live_rows = set(dispatch_state.live_row_ids())
    for row_id in delegated:
        if row_id in live_rows:
            state_parts.append(f"[{row_id}: dispatch still running]")
            continue  # still building -- not landed, not stuck
        row = dispatch_state.latest_row(row_id)
        if row is None:
            # Delegated, no live dispatch process, and no dispatch-log.csv
            # row at all for this row -- the dispatcher itself was killed
            # abruptly (reboot, OOM, kill -9) before it ever reached its own
            # finally block (which always appends a row, even on CRASH).
            trigger = join_trigger(
                trigger,
                f"row {row_id} was delegated but no dispatch ever completed "
                "(dispatcher likely killed abruptly) -- reset status: todo "
                "so it redispatches fresh")
            continue
        status, cost = row["status"], row["cost_usd"]
        report_path = dispatch_state.self_report_path_for(row_id, row["run_id"])
        state_parts.append(f"[{row_id}: {status} ${cost}]")
        if status == "GREEN":
            trigger = join_trigger(
                trigger,
                f"row {row_id} is GREEN with a verified patch at "
                f"{row.get('patch_path', '')} -- land it: uv run --project "
                f".claude/scripts .claude/scripts/land_lane.py land-patch "
                f"{row_id} {row.get('patch_path', '')}")
        elif status in ("STUCK", "RED"):
            # The model's own real-time explanation of what blocked it --
            # read directly, no transcript reconstruction needed (see
            # agent_loop.py's self_report_blocker and
            # plans/simple-dispatch-design.md's "Course correction" section
            # for why this replaced a separate LLM reviewer).
            self_report = ""
            if report_path and Path(report_path).is_file():
                self_report = f" -- agent's own report: {Path(report_path).read_text(errors='replace')}"
            trigger = join_trigger(
                trigger,
                f"row {row_id} is {status} (cost ${cost}){self_report} -- "
                "decide: narrower redispatch per the report, or a decide-row escalation")
        elif status == "TIMEOUT":
            trigger = join_trigger(
                trigger,
                f"row {row_id} timed out (cost ${cost}) -- likely an undersized "
                "budget, not unsolvable; consider one retry with a larger "
                "--overall-timeout before escalating")
        elif status == "SETUP_FAILED":
            trigger = join_trigger(
                trigger,
                f"row {row_id} failed to even start (host/sandbox setup issue, "
                f"cost ${cost}) -- plausibly transient (network, sandbox boot); "
                "worth one plain retry")
        elif status == "FATAL":
            trigger = join_trigger(
                trigger,
                f"row {row_id} hit FATAL (bad key or no OpenRouter credit) -- "
                "fix the account before redispatching anything")

    # Landing failures (serial queue, 2026-09-01): land_lane.py handles its
    # own conflict/red re-dispatches (cap 2); a marker here means the cap is
    # spent (or the main checkout stayed busy) and the tick must route it.
    # Tier 6 so a blocked landing of finished work always outranks routine
    # lane chatter (the lesson of the accumulator era: promotion triggers
    # starved behind dead-lane rebriefs all night, 2026-08-31).
    dead_priority = -1
    dead_trigger = ""
    for marker in sorted(LOG_DIR.glob("land-failed-issue-*")):
        n = marker.name.removeprefix("land-failed-issue-")
        content = marker.read_text(errors="replace").strip()
        state_parts.append(f"[land-failed: issue-{n} -- {content}]")
        if 6 > dead_priority:
            dead_priority = 6
            dead_trigger = f"landing of issue-{n} is stuck ({content}) -- route it"
    if not trigger and dead_trigger:
        trigger = dead_trigger

    # Maintainer input always runs the tick (the 5-minute quiet rule is
    # judged inside).
    if maintainer_input_pending():
        trigger = trigger or "maintainer input pending"

    # Decide starvation: the maintainer keeps checking an empty inbox while
    # decide rows sit ready. NOT a fallback (it starved twice, 2026-08-30:
    # as a fallback it lost to every landing and dead-lane trigger, and the
    # maintainer drained both buffered rounds in ten minutes with nothing
    # refilling) -- an under-filled round buffer ALWAYS joins the trigger,
    # alongside whatever else the tick has.
    starve = round_starvation_trigger()
    if starve:
        trigger = join_trigger(trigger, starve)

    # A free dispatcher with a ready row means dispatch is due. This JOINS
    # the trigger instead of being a fallback: as a fallback it starved 2h
    # behind the streak/starvation triggers while lanes sat idle
    # (2026-08-30, under the old model -- the reasoning still applies).
    # simple_dispatch.py's own ThreadPoolExecutor pool size IS the
    # concurrency limit for one call, so "occupied" is now a simple binary
    # (a live simple_dispatch.py process, or not) rather than a slot count.
    dispatch = dispatch_state.dispatch_trigger(dispatch_state.DEFAULT_CAP)
    if dispatch:
        trigger = join_trigger(trigger, dispatch)

    # Exhaustion: nothing delegated, nothing ready to build -- the idle
    # exception (drive skill) lets the tick self-originate one or two
    # exploration rows.
    if not trigger:
        exhausted = exhaustion_trigger()
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
    'orphans. Fix what is mechanical, file issues for the rest. End quietly '
    'if clean.'
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
    'records whose page is a docs/.grill/*.round.yaml are wizard submissions: '
    'apply IMMEDIATELY, delete the round file at capture. (2) If the trigger '
    'names an under-filled round buffer, compose the next wizard round BEFORE '
    'any landing (an empty maintainer inbox outranks dispatch plumbing): read '
    'pending rounds first and never re-ask them; keep two buffered; write '
    'docs/.grill/<topic>.round.yaml -- 3-5 ready decide rows batched by theme, '
    'every option carrying real verified code examples (delegate heavy example '
    'prep to a research worker) -- and ALWAYS verify the finished file both '
    'parses (python3 yaml.safe_load) AND serves clean (curl -s '
    'http://localhost:5173/__grill/round?topic=<topic>, expect 200) before the '
    'tick ends -- yaml.safe_load alone missed a round with valid YAML but no '
    '"question" string per question, which the mail UI rejected and which, '
    'until the isolation fix (kantord/toylang#164), blanked every OTHER '
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
    'non-GREEN outcome (STUCK, RED, TIMEOUT, SETUP_FAILED, FATAL) already '
    'carries the agents own real explanation of what blocked it, verbatim, in '
    'the trigger text -- read that directly and act on the per-status '
    'guidance already there; there is no event log or ESCALATION.md to '
    'reconstruct anymore. FATAL means the account itself needs fixing -- flag '
    'it plainly, do not redispatch anything until it is. Record a real, '
    'surprising incident (a wrong self-report, a repeated failure shape, a '
    'cost anomaly) as a note in plans/simple-dispatch-design.md, not a new '
    'file. RULES: never edit a repo file yourself to fix a build row -- '
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
        try:
            for line in proc.stdout:
                if tick_stream.process_line(line, str(out_path)):
                    break
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
            print(f"[drive-tick] {datetime.now():%H:%M:%S} {streak} consecutive "
                  f"auth failures -- wrote {down_file}")
            if first_detection:
                env = dict(os.environ)
                env.setdefault("DISPLAY", ":0")
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
        print(f"[drive-tick] {datetime.now():%H:%M:%S} another tick holds the "
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
    sandbox_gc_log = LOG_DIR / "sandbox-gc.log"
    try:
        with open(sandbox_gc_log, "a") as f:
            for line in dispatch_state.gc_orphaned_sandboxes():
                f.write(f"removed orphaned sandbox: {line}\n")
    except Exception as e:
        with open(sandbox_gc_log, "a") as f:
            f.write(f"gc failed (non-fatal): {e}\n")

    revive_dev_server()

    trigger, state = compute_trigger_and_state(mode)
    if not trigger:
        print(f"[drive-tick] {datetime.now():%H:%M:%S} nothing to do (workers "
              "grinding, no input) -- skipped, zero tokens")
        return 0

    policy = AUDIT_POLICY if mode == "audit" else TICK_POLICY

    ts = time.strftime("%Y%m%d-%H%M%S")
    out_path = LOG_DIR / f"{ts}-{mode}-{MODEL}.json"
    print(f"[drive-tick] {datetime.now():%H:%M:%S} {mode} starting on {MODEL} "
          f"-- {trigger} (log: {out_path})")

    try:
        inbox_n = str(len(json.loads((REPO / "docs/.annotations/inbox.json").read_text())
                          .get("records", [])))
    except (OSError, json.JSONDecodeError):
        inbox_n = "?"
    rounds = " ".join(sorted(p.name for p in (REPO / "docs" / ".grill").glob("*.round.yaml")))
    core = (f"Trigger: {trigger}. Snapshot (from disk this second -- act on it, "
            f"re-verify only what you modify):{state or ' no delegated rows'} "
            f"[inbox_records={inbox_n} pending_rounds={rounds or 'none'}]. You are "
            "a ROUTER: turns are for decisions and the four scripts "
            "(simple_dispatch.py, land_lane.py, board-archive.py, round files), "
            "never exploration. Nothing else dispatches build work -- "
            "sandbox_dispatch.py, dispatch-worker.sh, and opencode are retired.")

    run_tick(f"{policy} {core}", out_path)
    check_coordinator_auth(out_path)
    return 0


if __name__ == "__main__":
    sys.exit(main())
