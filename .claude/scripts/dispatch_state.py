#!/usr/bin/env python3
"""State/liveness helper for simple_dispatch.py, imported directly by
drive_tick.py (same process, no subprocess round-trip -- both are plain
Python in the same uv project now) and also runnable standalone for a human
via `uv run --project .claude/scripts .claude/scripts/dispatch_state.py ...`.

Replaces sandbox_dispatch_status.py, which was keyed entirely to the old
sandbox_dispatch.py/opencode pipeline's conventions (one process per row,
a live-worktree-per-lane model, ESCALATION.md files, jsonl event logs).
simple_dispatch.py has none of that: a single process handles a whole
--parallel batch at once and reports through two plain, structured
surfaces -- plans/dispatch-log.csv (one row per run) and
~/.cache/toylang-simple-dispatch/results/<row>-<run_id>-* files (patch,
full log, self-report, persisted messages). This script reads those
directly instead of reconstructing state from worktrees/pgrep/markers.

Usage:
  uv run --project .claude/scripts .claude/scripts/dispatch_state.py --live
      Prints the row ids currently being processed by a live
      simple_dispatch.py invocation (found by scanning cmdlines), one per
      line. Empty output means no dispatch is running right now -- the
      dispatcher is a single global batch, not a per-row slot pool, so
      "busy or free" is the whole WIP model; --parallel controls fan-out
      INSIDE one call, not how many calls can run at once.
  uv run --project .claude/scripts .claude/scripts/dispatch_state.py --status ROW_ID
      Prints the latest plans/dispatch-log.csv row for ROW_ID as
      "status cost_usd patch_path self_report_path" (space-separated,
      paths empty-string if absent), or nothing if no row exists yet.
  uv run --project .claude/scripts .claude/scripts/dispatch_state.py --dispatch-trigger [--cap N]
      Prints a one-line trigger message if no dispatch is currently live
      AND at least one ready `build` row exists in plans/board.yaml
      (status: todo, kind: build, every `needs` id either absent from the
      live board or not still todo/delegated) -- naming up to N of them.
      Empty output otherwise.
  uv run --project .claude/scripts .claude/scripts/dispatch_state.py --gc
      Removes any `msb` sandbox whose name matches the sd-<row>-<runid>
      pattern but has no corresponding live simple_dispatch.py process.
      simple_dispatch.py's own teardown already does this in the normal
      case; this only catches a sandbox orphaned by an abrupt kill of the
      dispatcher itself (reboot, OOM, kill -9) -- rare, but simple_dispatch.py's
      own crash-tolerant teardown can't run if the whole process is gone.
"""
import csv
import re
import subprocess
import sys
from pathlib import Path

REPO = Path("/home/kantord/repos/toylang")
DISPATCH_LOG = REPO / "plans" / "dispatch-log.csv"
RESULT_DIR = Path.home() / ".cache" / "toylang-simple-dispatch" / "results"
MSB_BIN = Path.home() / ".local/bin/msb"
DEFAULT_CAP = 3


def live_row_ids() -> list[str]:
    """Row ids currently being processed by a live simple_dispatch.py
    invocation, found by reading each process's REAL argv tokens directly
    from /proc/<pid>/cmdline -- not a state file, which can go stale
    exactly like the old board.yaml-status/msb-list staleness this whole
    design already learned to distrust once.

    Deliberately NOT `pgrep -af simple_dispatch.py`: that matches as a
    substring against the WHOLE command line joined into one string, which
    false-positives on any process whose command line merely MENTIONS
    "simple_dispatch.py" in prose. Confirmed live, 2026-09-11: a running
    `claude -p` tick's own POLICY prompt text quotes "simple_dispatch.py"
    verbatim many times as ONE giant argv string -- calling this function
    from within that tick's own turn (a real Bash tool call the coordinator
    made to check dispatcher status) matched the tick's own PID and parsed
    its entire prompt into garbage row-id tokens. Reading real, NUL-
    separated argv per process and requiring an EXACT token match (the
    literal script name, not a substring anywhere in a longer argument)
    can't collide with prose that merely contains the same words."""
    ids: list[str] = []
    for pid_dir in Path("/proc").glob("[0-9]*"):
        try:
            raw = pid_dir.joinpath("cmdline").read_bytes()
        except OSError:
            continue  # process exited between the glob and the read
        argv = [a.decode(errors="replace") for a in raw.split(b"\0") if a]
        script_at = next((i for i, tok in enumerate(argv)
                           if tok == "simple_dispatch.py" or tok.endswith("/simple_dispatch.py")),
                          None)
        if script_at is None:
            continue
        # Row ids are simple_dispatch.py's own positional args, which always
        # come immediately after the script path and before its first flag
        # (--brief-dir, --parallel, ...) -- this correctly ignores whatever
        # precedes the script path (a bare `python3`, or `uv run --project
        # <dir>`), without needing to special-case every possible launcher
        # prefix by name.
        for tok in argv[script_at + 1:]:
            if tok.startswith("-"):
                break
            ids.append(tok)
    return ids


def latest_row(row_id: str) -> dict | None:
    if not DISPATCH_LOG.exists():
        return None
    best = None
    with open(DISPATCH_LOG, newline="") as f:
        for row in csv.DictReader(f):
            if row.get("row_id") != row_id:
                continue
            if best is None or row["start_time"] > best["start_time"]:
                best = row
    return best


def self_report_path_for(row_id: str, run_id: str) -> str:
    p = RESULT_DIR / f"{row_id}-{run_id}-self-report.txt"
    return str(p) if p.exists() else ""


def dispatch_trigger(cap: int) -> str | None:
    if live_row_ids():
        return None
    import yaml
    rows = yaml.safe_load(open(REPO / "plans" / "board.yaml"))
    live_ids = {r["id"] for r in rows if r.get("status") in ("todo", "delegated")}

    def is_ready(r):
        if r.get("kind") != "build" or r.get("status") != "todo":
            return False
        return all(n not in live_ids for n in r.get("needs", []))

    ready = [r["id"] for r in rows if is_ready(r)]
    if not ready:
        return None
    return f"dispatcher free, ready build row(s): {' '.join(ready[:cap])}"


def gc_orphaned_sandboxes() -> list[str]:
    try:
        r = subprocess.run([str(MSB_BIN), "list"], text=True, capture_output=True, timeout=30)
    except (subprocess.SubprocessError, OSError):
        return []
    live = set(live_row_ids())
    removed = []
    for line in r.stdout.splitlines():
        m = re.match(r"\s*(sd-([A-Za-z0-9_-]+)-[0-9a-f]{8})\b", line)
        if not m:
            continue
        name, row_id = m.group(1), m.group(2)
        if row_id in live:
            continue
        rm = subprocess.run([str(MSB_BIN), "rm", "-f", name],
                             text=True, capture_output=True, timeout=60)
        if rm.returncode == 0:
            removed.append(name)
    return removed


if __name__ == "__main__":
    args = sys.argv[1:]
    if "--live" in args:
        for row_id in live_row_ids():
            print(row_id)
    elif "--status" in args:
        row_id = args[args.index("--status") + 1]
        row = latest_row(row_id)
        if row:
            report = self_report_path_for(row_id, row["run_id"])
            print(f"{row['status']} {row['cost_usd']} {row.get('patch_path', '')} {report}")
    elif "--dispatch-trigger" in args:
        cap = DEFAULT_CAP
        if "--cap" in args:
            cap = int(args[args.index("--cap") + 1])
        t = dispatch_trigger(cap)
        if t:
            print(t)
    elif "--gc" in args:
        for n in gc_orphaned_sandboxes():
            print(f"removed orphaned sandbox: {n}")
    else:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
