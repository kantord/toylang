#!/usr/bin/env python3
"""State, liveness, and health helper for simple_dispatch.py, imported
directly by drive_tick.py (same process, no subprocess round-trip -- both are
plain Python in the same uv project) and also runnable standalone for a human
via `uv run --project .claude/scripts .claude/scripts/dispatch_state.py ...`.

simple_dispatch.py reports through two plain, structured surfaces --
plans/dispatch-log.csv (one row per run) and one bundle directory per run,
~/.cache/toylang-simple-dispatch/results/<row>/<run_id>/ (status.json,
brief.txt, agent.log, messages.json, self-report.{txt,json}, patch,
dispatch.log). This script reads those directly instead of reconstructing
state from worktrees, pgrep, or markers. Runs before 2026-09-14 left flat
suffix-named files in results/ instead of a bundle; --show and --status
fall back to those where they exist.

Usage:
  dispatch_state.py --live
      Row ids currently being processed by a live simple_dispatch.py
      invocation (found by scanning cmdlines), one per line. Empty output
      means no dispatch is running right now -- the dispatcher is a single
      global batch, not a per-row slot pool.
  dispatch_state.py --live-landings
      Row ids named by a live land_lane.py process.
  dispatch_state.py --status ROW_ID
      Latest dispatch-log.csv row for ROW_ID as
      "status cost_usd patch_path self_report_path ended_by edits turns".
  dispatch_state.py --show ROW_ID [RUN_ID]
      THE standard way to look at a run that did not go GREEN: status.json,
      each attempt's ending and verify result, the self-report, and the
      last 30 lines of the agent log. Defaults to the latest run.
  dispatch_state.py --health [--last N]
      Population view over the last N runs (default 20): how each ended
      (ended_by), the zero-edit rate, the GREEN rate, and whether the
      harness-defect alarm is on. This is what the 2026-09-14 incident
      lacked -- 46 of 54 runs ended by the harness's own turn cutoff with
      zero edits, and every one was judged in isolation as a task problem.
  dispatch_state.py --dispatch-trigger [--cap N]
      One-line trigger if no dispatch is live AND ready `build` rows exist.
  dispatch_state.py --capture ROW_ID [RUN_ID]
      Copy the decision-relevant slice of a run (status.json, self-report,
      last 100 log lines) into plans/incidents/<row>-<date>/ so the evidence
      behind an escalation survives cache cleanup and is readable from git.
  dispatch_state.py --health-ack "NOTE"
      Record that the harness was fixed now; --health counts only runs
      started after this, so a fix can be proven by the next runs instead
      of drowned by the old ones.
  dispatch_state.py --backfill
      One-off: fill ended_by/edits/turns for rows that predate status.json
      from their flat agent logs (ended_by inferred from the log's own
      marker lines; edits = attempts that ran a real verify).
  dispatch_state.py --gc
      Remove msb sandboxes orphaned by an abrupt dispatcher kill, and old
      bundles past the retention rule (keep the last 3 per row, plus every
      bundle whose row is still delegated or named in a pending round).
"""
from __future__ import annotations

import csv
import json
import re
import shutil
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path

REPO = Path("/home/kantord/repos/toylang")
DISPATCH_LOG = REPO / "plans" / "dispatch-log.csv"
RESULT_DIR = Path.home() / ".cache" / "toylang-simple-dispatch" / "results"
INCIDENT_DIR = REPO / "plans" / "incidents"
ROUND_DIR = REPO / "docs" / ".grill"
MSB_BIN = Path.home() / ".local/bin/msb"
DEFAULT_CAP = 3
KEEP_BUNDLES_PER_ROW = 3

# Endings where the harness stopped a run that still had budget on paper:
# these say nothing about the task. `max_turns` and `dedup` are NOT here --
# a run that used all 30 turns twice and changed nothing had its full
# budget; that is a convergence problem for the coordinator's verbs (a)/(b),
# as the select-lazy-materialization family showed on 2026-09-15 once the
# real harness bugs were gone. Three of these in a row, or a majority of
# zero-edit runs, is a pipeline defect until proven otherwise.
HARNESS_ENDINGS = ("no_progress_cutoff", "wall_clock", "reasoning_exhausted")
HEALTH_WINDOW = 20
# `--health-ack` records "the harness was fixed at this time"; runs before
# it no longer count toward the alarm (they still show in --show). Without
# this the alarm that a fix was meant to answer would keep firing, and
# keep telling ticks to hold dispatch, until 20 new runs had diluted it.
HEALTH_ACK_FILE = Path.home() / ".cache" / "toylang-simple-dispatch" / "health-ack.json"
ZERO_EDIT_ALARM_RATE = 0.5
CONSECUTIVE_HARNESS_ALARM = 3


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
    from within that tick's own turn matched the tick's own PID and parsed
    its entire prompt into garbage row-id tokens."""
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
        # Row ids are simple_dispatch.py's own positional args, right after
        # the script path and before its first flag. A `--preflight` run has
        # none, and correctly contributes nothing here.
        for tok in argv[script_at + 1:]:
            if tok.startswith("-"):
                break
            ids.append(tok)
    # `uv run ... simple_dispatch.py` is two matching processes (the uv
    # supervisor plus the venv python it execs) with the same argv.
    return list(dict.fromkeys(ids))


def live_land_row_ids() -> list[str]:
    """Rows named by a live land_lane.py process (land-patch <row> or land
    <row>). The tick must not launch a second landing for these -- a tick
    did, three times over, on 2026-09-14."""
    ids: list[str] = []
    for pid_dir in Path("/proc").glob("[0-9]*"):
        try:
            raw = pid_dir.joinpath("cmdline").read_bytes()
        except OSError:
            continue
        argv = [a.decode(errors="replace") for a in raw.split(b"\0") if a]
        at = next((i for i, t in enumerate(argv) if t.endswith("land_lane.py")), None)
        if at is None or len(argv) < at + 3 or argv[at + 1] not in ("land", "land-patch"):
            continue
        ids.append(argv[at + 2].removeprefix("issue-"))
    return list(dict.fromkeys(ids))


def all_rows() -> list[dict]:
    if not DISPATCH_LOG.exists():
        return []
    with open(DISPATCH_LOG, newline="") as f:
        return [r for r in csv.DictReader(f) if r.get("run_id")]


def latest_row(row_id: str) -> dict | None:
    best = None
    for row in all_rows():
        if row.get("row_id") != row_id:
            continue
        if best is None or row["start_time"] > best["start_time"]:
            best = row
    return best


def bundle_for(row_id: str, run_id: str) -> Path:
    return RESULT_DIR / row_id / run_id


def self_report_path_for(row_id: str, run_id: str) -> str:
    for p in (bundle_for(row_id, run_id) / "self-report.txt",
              RESULT_DIR / f"{row_id}-{run_id}-self-report.txt"):
        if p.exists():
            return str(p)
    return ""


def agent_log_path_for(row_id: str, run_id: str) -> Path | None:
    for p in (bundle_for(row_id, run_id) / "agent.log",
              RESULT_DIR / f"{row_id}-{run_id}-full-agent.log"):
        if p.exists():
            return p
    return None


def read_status_json(row_id: str, run_id: str) -> dict | None:
    p = bundle_for(row_id, run_id) / "status.json"
    if not p.exists():
        return None
    try:
        return json.loads(p.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def show(row_id: str, run_id: str | None = None) -> str:
    row = latest_row(row_id) if run_id is None else next(
        (r for r in all_rows() if r.get("row_id") == row_id and r.get("run_id") == run_id), None)
    if row is None:
        return f"{row_id}: no dispatch recorded" + (f" for run {run_id}" if run_id else "")
    run_id = row["run_id"]
    out = [f"{row_id} run {run_id}: {row['status']} ${row['cost_usd']} "
           f"ended_by={row.get('ended_by') or '(pre-2026-09-14, not recorded)'} "
           f"edits={row.get('edits') or '?'} turns={row.get('turns') or '?'} "
           f"model={row['model']} {row['start_time']} -> {row['end_time']} ({row['duration_s']}s)"]
    status = read_status_json(row_id, run_id)
    if status:
        out.append(f"  snapshot={status.get('snapshot')} base={str(status.get('base_commit'))[:9]} "
                   f"patch={'yes' if status.get('patch') else 'no'}")
        for a in status.get("attempts") or []:
            out.append(f"  attempt {a.get('n')}: ended_by={a.get('ending')} turns={a.get('turns')} "
                       f"edits={a.get('edits')} verify={a.get('verify') or '-'}")
            if a.get("verify_tail"):
                out.append("    " + a["verify_tail"].strip().splitlines()[-1][:200])
        sr = status.get("self_report")
        if sr:
            out.append(f"  self-report [{sr.get('blocker_kind')}, narrower_would_succeed="
                       f"{sr.get('narrower_would_succeed')}]: {sr.get('explanation')}")
    else:
        out.append(f"  (no status.json -- pre-bundle run; files: "
                   f"{', '.join(p.name for p in RESULT_DIR.glob(f'{row_id}-{run_id}*')) or 'none'})")
        rp = self_report_path_for(row_id, run_id)
        if rp:
            out.append(f"  self-report: {Path(rp).read_text(errors='replace').strip()}")
    if row.get("patch_path"):
        out.append(f"  patch: {row['patch_path']}")
    log = agent_log_path_for(row_id, run_id)
    if log:
        lines = log.read_text(errors="replace").splitlines()
        out.append(f"  agent log ({log}, last 30 of {len(lines)} lines):")
        out += ["    " + line[:200] for line in lines[-30:]]
    return "\n".join(out)


def health(last: int = HEALTH_WINDOW) -> dict:
    """The population view. Only runs that recorded ended_by count toward
    the alarm; older rows are reported as `unrecorded` so a mostly-old log
    reads as "not enough data", never as "healthy"."""
    return health_from_rows(all_rows(), read_health_ack(), last)


def health_from_rows(rows: list[dict], ack: dict | None, last: int = HEALTH_WINDOW) -> dict:
    rows = sorted(rows, key=lambda r: r["start_time"])
    if ack:
        rows = [r for r in rows if r["start_time"] >= ack["time"]]
    rows = rows[-last:]
    recorded = [r for r in rows if r.get("ended_by")]
    endings: dict[str, int] = {}
    for r in recorded:
        endings[r["ended_by"]] = endings.get(r["ended_by"], 0) + 1
    zero_edit = [r for r in recorded if (r.get("edits") or "0") == "0" and r["status"] != "GREEN"]
    green = [r for r in rows if r["status"] == "GREEN"]
    # A GREEN run breaks the streak whatever ended its last attempt: a run
    # that hit max_turns and then passed verify is a success, not a
    # harness casualty (the first fixed-harness run did exactly that).
    streak = 0
    for r in reversed(recorded):
        if r["ended_by"] in HARNESS_ENDINGS and r["status"] != "GREEN":
            streak += 1
        else:
            break
    reasons = []
    if recorded and len(zero_edit) / len(recorded) > ZERO_EDIT_ALARM_RATE:
        reasons.append(f"{len(zero_edit)} of {len(recorded)} recorded runs changed nothing")
    if streak >= CONSECUTIVE_HARNESS_ALARM:
        reasons.append(f"last {streak} runs were ended by the harness "
                       f"({', '.join(r['ended_by'] for r in recorded[-streak:])})")
    return {
        "window": len(rows), "recorded": len(recorded), "unrecorded": len(rows) - len(recorded),
        "endings": endings, "zero_edit": len(zero_edit), "green": len(green),
        "harness_streak": streak, "alarm": bool(reasons), "reasons": reasons,
        "zero_edit_rows": [r["row_id"] for r in zero_edit],
        "ack": ack,
    }


def read_health_ack() -> dict | None:
    try:
        return json.loads(HEALTH_ACK_FILE.read_text())
    except (OSError, json.JSONDecodeError):
        return None


def write_health_ack(note: str) -> dict:
    HEALTH_ACK_FILE.parent.mkdir(parents=True, exist_ok=True)
    ack = {"time": datetime.now(UTC).isoformat(), "note": note}
    HEALTH_ACK_FILE.write_text(json.dumps(ack, indent=1))
    return ack


def format_health(h: dict) -> str:
    endings = ", ".join(f"{k}={v}" for k, v in sorted(h["endings"].items())) or "none recorded"
    since = f" since ack {h['ack']['time'][:16]} ({h['ack']['note']})" if h.get("ack") else ""
    line = (f"dispatch health, last {h['window']} runs{since}: GREEN {h['green']}, zero-edit {h['zero_edit']}, "
            f"ended_by {{{endings}}}, unrecorded(pre-2026-09-14) {h['unrecorded']}, "
            f"harness streak {h['harness_streak']}")
    if h["alarm"]:
        line += " -- ALARM: " + "; ".join(h["reasons"])
    return line


def health_trigger(last: int = HEALTH_WINDOW) -> str | None:
    """The tick's third verb. Everything before this offered two: redispatch
    narrower, or escalate the row. Neither can say "the tool is broken"."""
    h = health(last)
    if not h["alarm"]:
        return None
    return (format_health(h) + " -- this is a HARNESS DEFECT signal, not a task-scope one: do not "
            "escalate individual rows or compose round questions about them; hold dispatch, "
            "capture the evidence (dispatch_state.py --capture ROW), and open or update ONE harness "
            "decide row naming the common ended_by (plans/dispatch-self-healing-plan.md)")


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


def capture_incident(row_id: str, run_id: str | None = None) -> Path | None:
    """plans/incidents/<row>-<date>/ already exists as the place a decision's
    evidence lives in git. The full log stays in the cache; this holds the
    shape: status.json, the self-report, the last 100 log lines."""
    row = latest_row(row_id) if run_id is None else None
    if row_id and run_id is None:
        if row is None:
            return None
        run_id = row["run_id"]
    assert run_id is not None
    dest = INCIDENT_DIR / f"{row_id}-{datetime.now(UTC).strftime('%Y%m%d')}"
    dest.mkdir(parents=True, exist_ok=True)
    bundle = bundle_for(row_id, run_id)
    for name in ("status.json", "self-report.json", "self-report.txt"):
        src = bundle / name
        if src.exists():
            shutil.copyfile(src, dest / name)
    if not (dest / "self-report.txt").exists():
        flat = self_report_path_for(row_id, run_id)
        if flat:
            shutil.copyfile(flat, dest / "self-report.txt")
    log = agent_log_path_for(row_id, run_id)
    if log:
        lines = log.read_text(errors="replace").splitlines()
        (dest / "agent-log-tail.txt").write_text("\n".join(lines[-100:]) + "\n")
    (dest / "run.txt").write_text(f"{row_id} {run_id}\n{show(row_id, run_id)}\n")
    return dest


def infer_ending_from_log(text: str) -> tuple[str, int, int]:
    """(ended_by, edits, turns) for a run that predates status.json, read
    from the shape of its agent log. `edits` here is attempts that ran a
    real verify (i.e. changed the tree), not per-turn edits -- the best the
    old logs can say. Used once by --backfill so --health has history to
    stand on instead of 54 "unrecorded" rows."""
    attempts = text.count("== attempt ")
    no_change = text.count("(no changes, no verify run)")
    turns = len(re.findall(r"^\s*turn \d+/\d+: prompt=", text, re.M))
    if "VERIFIED_GREEN" in text:
        ending = "model_done"
    elif "STUCK: verify output matches a previous attempt" in text:
        ending = "dedup"
    elif "OUT_OF_TIME" in text:
        ending = "wall_clock"
    elif "no repo changes for" in text and "ending this attempt early" in text:
        ending = "no_progress_cutoff"
    elif "ran out of turns without the model finishing" in text:
        ending = "max_turns"
    elif "call failed" in text:
        ending = "api_error"
    else:
        ending = ""
    return ending, max(0, attempts - no_change), turns


def backfill_endings() -> int:
    """Fill empty ended_by/edits/turns columns for old rows from their
    flat agent logs. Returns the number of rows updated."""
    rows = all_rows()
    if not rows:
        return 0
    fields = list(rows[0].keys())
    for col in ("bundle_path", "ended_by", "edits", "turns"):
        if col not in fields:
            fields.append(col)
    updated = 0
    for r in rows:
        if r.get("ended_by"):
            continue
        log = agent_log_path_for(r["row_id"], r["run_id"])
        if not log:
            continue
        ending, edits, turns = infer_ending_from_log(log.read_text(errors="replace"))
        if not ending:
            continue
        r["ended_by"], r["edits"], r["turns"] = ending, str(edits), str(turns)
        updated += 1
    tmp = DISPATCH_LOG.with_suffix(".csv.tmp")
    with open(tmp, "w", newline="") as f:
        w = csv.DictWriter(f, fieldnames=fields)
        w.writeheader()
        for r in rows:
            w.writerow({k: r.get(k, "") or "" for k in fields})
    tmp.replace(DISPATCH_LOG)
    return updated


def protected_row_ids() -> set[str]:
    import yaml
    protected = set()
    try:
        for r in yaml.safe_load(open(REPO / "plans" / "board.yaml")):
            if r.get("status") == "delegated":
                protected.add(r["id"])
    except (OSError, yaml.YAMLError):
        pass
    for rf in ROUND_DIR.glob("*.forest.yaml"):
        try:
            text = rf.read_text()
        except OSError:
            continue
        for row_dir in RESULT_DIR.iterdir() if RESULT_DIR.exists() else []:
            if row_dir.is_dir() and row_dir.name in text:
                protected.add(row_dir.name)
    return protected


def gc_bundles() -> list[str]:
    """Keep the last KEEP_BUNDLES_PER_ROW bundles per row, and everything
    for a row that is still delegated or named in a pending round."""
    removed = []
    if not RESULT_DIR.exists():
        return removed
    protected = protected_row_ids()
    for row_dir in RESULT_DIR.iterdir():
        if not row_dir.is_dir() or row_dir.name in protected:
            continue
        runs = sorted((d for d in row_dir.iterdir() if d.is_dir()), key=lambda d: d.stat().st_mtime)
        for old in runs[:-KEEP_BUNDLES_PER_ROW]:
            shutil.rmtree(old, ignore_errors=True)
            removed.append(str(old))
    return removed


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


def _arg_after(args: list[str], flag: str, default=None):
    if flag in args and args.index(flag) + 1 < len(args):
        return args[args.index(flag) + 1]
    return default


def _positional_after(args: list[str], flag: str) -> list[str]:
    out = []
    for tok in args[args.index(flag) + 1:]:
        if tok.startswith("-"):
            break
        out.append(tok)
    return out


if __name__ == "__main__":
    args = sys.argv[1:]
    if "--live" in args:
        for row_id in live_row_ids():
            print(row_id)
    elif "--live-landings" in args:
        for row_id in live_land_row_ids():
            print(row_id)
    elif "--status" in args:
        row_id = args[args.index("--status") + 1]
        row = latest_row(row_id)
        if row:
            report = self_report_path_for(row_id, row["run_id"])
            print(f"{row['status']} {row['cost_usd']} {row.get('patch_path', '')} {report} "
                  f"{row.get('ended_by', '')} {row.get('edits', '')} {row.get('turns', '')}")
    elif "--show" in args:
        pos = _positional_after(args, "--show")
        if not pos:
            print("--show needs ROW_ID [RUN_ID]", file=sys.stderr)
            sys.exit(2)
        print(show(pos[0], pos[1] if len(pos) > 1 else None))
    elif "--health" in args:
        n = int(_arg_after(args, "--last", HEALTH_WINDOW))
        h = health(n)
        print(format_health(h))
        if h["zero_edit_rows"]:
            print("zero-edit rows: " + " ".join(dict.fromkeys(h["zero_edit_rows"])))
        sys.exit(1 if h["alarm"] else 0)
    elif "--capture" in args:
        pos = _positional_after(args, "--capture")
        if not pos:
            print("--capture needs ROW_ID [RUN_ID]", file=sys.stderr)
            sys.exit(2)
        dest = capture_incident(pos[0], pos[1] if len(pos) > 1 else None)
        print(dest if dest else f"{pos[0]}: nothing to capture")
    elif "--dispatch-trigger" in args:
        cap = int(_arg_after(args, "--cap", DEFAULT_CAP))
        t = dispatch_trigger(cap)
        if t:
            print(t)
    elif "--health-ack" in args:
        note = _arg_after(args, "--health-ack", "") or ""
        print(json.dumps(write_health_ack(note)))
    elif "--backfill" in args:
        print(f"backfilled {backfill_endings()} rows from their agent logs")
    elif "--gc" in args:
        for n in gc_orphaned_sandboxes():
            print(f"removed orphaned sandbox: {n}")
        for b in gc_bundles():
            print(f"removed old bundle: {b}")
    else:
        print(__doc__, file=sys.stderr)
        sys.exit(2)
