#!/usr/bin/env python3
"""Schema validator for plans/board.yaml and plans/board-archive.yaml.

The board has several deterministic writers now (ticks, board-archive.py,
humans), and the site renders whatever they wrote: a row
missing a field the UI dereferences blanked the whole kanban to empty
(needs-less rows, 2026-09-01). Parsing is not validity -- this is the schema
gate, run by every writer before committing and by .claude/checks at Stop.

`blocked_by` (2026-09-13): a row can be genuinely not-dispatchable for a
reason `needs:` can't express -- no other row id to point at (an unbuilt
mechanism with no row of its own yet), a maintainer ruling to hand off
manually, or similar. Before this field existed the only place that fact
lived was the row's own free-text title, which `is_ready()` in
dispatch_state.py never reads: dsv-partials-migration and
euler-slow-fragments-2 both sat at status: todo (mechanically "ready",
needs satisfied) for 6+ ticks across several hours, each one independently
re-reading the title, agreeing it's blocked, and skipping -- without ever
fixing the status field, so the next tick paid the same cost again. Setting
`blocked_by` makes that fact structural instead of prose a human has to
notice: if it's set, status must not be `todo` (enforced below), so a row
can't silently sit "ready" while also declaring itself not ready.

Stale delegated rows and escalation caps (2026-09-14): 12 rows sat at
`status: delegated` for two days while nothing was running -- each one's
last plans/dispatch-log.csv row was a terminal STUCK, and no tick reset
them because `delegated` reads as "someone else's problem" to every reader.
In the same stretch docs/.grill/stuck-row-triage-2.round.yaml grew to seven
escalation questions about one harness bug, because nothing capped how many
STUCK rows a round may ask a human to rule on one by one. Both are now
findings here; the reasoning is in plans/dispatch-self-healing-plan.md.

Exit 0 = valid. Exit 1 = findings on stderr, one per line.
"""

import csv
import subprocess
import sys
from datetime import UTC, datetime, timedelta
from pathlib import Path

import yaml

SCRIPTS_DIR = Path(__file__).resolve().parent
BOARD = Path("plans/board.yaml")
ARCHIVE = Path("plans/board-archive.yaml")
DISPATCH_LOG = Path("plans/dispatch-log.csv")
GRILL_DIR = Path("docs/.grill")
MEMORY = Path("plans/coordinator-memory.yaml")

KINDS = {"build", "decide"}
STATUSES = {"todo", "delegated", "done", "proposed"}

# simple_dispatch.py appends its CSV row only after a run has ended, so every
# status it writes today is terminal. Named so the stale check stays honest
# if an in-flight marker is ever added: an unknown status is not flagged.
TERMINAL_STATUSES = {"GREEN", "RED", "STUCK", "TIMEOUT", "SETUP_FAILED", "FATAL", "CRASH"}

# The drive loop ticks every 10 minutes and each tick is supposed to reset or
# redispatch a finished row; six hours is dozens of missed ticks, past any
# gap a human pausing the loop for an afternoon would leave, and well short
# of the two days the 2026-09-13 rows actually sat.
STALE_DELEGATED_AFTER = timedelta(hours=6)

# More STUCK rows than this in one round means the harness broke, not that
# four separate scope decisions came due at once (plans/dispatch-self-healing-plan.md).
ESCALATION_CAP = 4

# plans/coordinator-memory.yaml (plans/coordinator-memory-design.md): facts a
# tick would otherwise re-derive, plus `watch` -- a condition to check every
# tick. The enum is the scope rule: incident and procedure narrative has a
# home in plans/simple-dispatch-design.md, and a `kind: stall-diagnosis`
# slot must fail here rather than quietly turn the pool into a second one.
# `watch` exists because the 2026-09-11 "worth watching" note about the
# no-progress cutoff sat in that design doc unread while the shape recurred
# in 46 of the next 54 runs (plans/dispatch-self-healing-plan.md).
MEMORY_KINDS = {"footprint-conflict", "fact-check", "watch", "other"}
MEMORY_CAP = 8

def lint(path, archived):
    errs = []
    try:
        rows = yaml.safe_load(open(path))
    except Exception as e:  # noqa: BLE001 -- any parse failure is the finding
        return [f"{path}: does not parse: {e}"]
    if not isinstance(rows, list):
        return [f"{path}: top level must be a list of rows"]
    seen = set()
    for i, r in enumerate(rows):
        where = f"{path}: row {i} ({r.get('id', '?') if isinstance(r, dict) else '?'})"
        if not isinstance(r, dict):
            errs.append(f"{where}: not a mapping")
            continue
        for field in ("id", "title"):
            if not isinstance(r.get(field), str) or not r.get(field).strip():
                errs.append(f"{where}: missing or empty '{field}'")
        if r.get("kind") not in KINDS:
            errs.append(f"{where}: kind must be one of {sorted(KINDS)}")
        if r.get("status") not in STATUSES:
            errs.append(f"{where}: status must be one of {sorted(STATUSES)}")
        if archived and r.get("status") != "done":
            errs.append(f"{where}: archive rows must be status: done")
        needs = r.get("needs", [])
        if not (isinstance(needs, list)
                and all(isinstance(n, str) for n in needs)):
            errs.append(f"{where}: needs must be a list of row ids")
        issue = r.get("issue")
        if issue is not None and not (isinstance(issue, str)
                                      and issue.startswith("gh:")):
            errs.append(f"{where}: issue must look like gh:<number>")
        blocked_by = r.get("blocked_by")
        if blocked_by is not None:
            if not (isinstance(blocked_by, str) and blocked_by.strip()):
                errs.append(f"{where}: blocked_by must be a non-empty string when present")
            if r.get("status") == "todo":
                errs.append(
                    f"{where}: status is todo but blocked_by is set -- "
                    f"a row can't be both dispatch-ready and declare itself blocked; "
                    f"set status to proposed or clear blocked_by"
                )
        if r["id"] in seen:
            errs.append(f"{where}: duplicate id")
        seen.add(r.get("id"))
    return errs

def live_dispatch_row_ids():
    """Row ids a running simple_dispatch.py is processing right now.

    Subprocess rather than `import dispatch_state`: this script imports no
    sibling module and is the gate every board writer runs, so it should
    not inherit the dispatcher's import surface. `--live` reads only /proc
    with the stdlib, so the interpreter running this script is enough --
    no nested `uv run`.
    """
    out = subprocess.run(
        [sys.executable, str(SCRIPTS_DIR / "dispatch_state.py"), "--live"],
        text=True, capture_output=True, check=True,
    ).stdout
    return set(out.split())


def latest_dispatch_rows(log_path):
    """Latest dispatch-log row per row_id, by start_time. Read by header so
    a run that wrote extra trailing columns, or an older one that wrote
    fewer, still counts."""
    latest = {}
    if not Path(log_path).exists():
        return latest
    with open(log_path, newline="") as f:
        for row in csv.DictReader(f):
            rid = row.get("row_id")
            if rid and (rid not in latest or row["start_time"] > latest[rid]["start_time"]):
                latest[rid] = row
    return latest


def lint_stale_delegated(rows, log_path, live_ids=live_dispatch_row_ids, now=None):
    """`live_ids` is a callable so the process scan only happens when some
    delegated row actually looks finished; `now` is injectable for tests."""
    now = now or datetime.now(UTC)
    latest = latest_dispatch_rows(log_path)
    candidates = []
    for r in rows:
        if r.get("status") != "delegated":
            continue
        last = latest.get(r["id"])
        if last is None:
            candidates.append((r["id"], None))
        elif (last["status"] in TERMINAL_STATUSES
              and now - datetime.fromisoformat(last["end_time"]) > STALE_DELEGATED_AFTER):
            candidates.append((r["id"], last))
    if not candidates:
        return []
    live = set(live_ids())
    errs = []
    for rid, last in candidates:
        if rid in live:
            continue
        if last is None:
            errs.append(f"{BOARD}: {rid}: delegated with no dispatch recorded")
        else:
            errs.append(
                f"{BOARD}: {rid}: delegated but its last dispatch ended {last['status']} "
                f"at {last['end_time']}, nothing live -- reset to todo or archive"
            )
    return errs


def lint_round_escalations(path):
    doc = yaml.safe_load(open(path))
    questions = doc.get("questions", []) if isinstance(doc, dict) else []
    n = sum(1 for q in questions if isinstance(q, dict) and q.get("flow") == "escalation")
    if n <= ESCALATION_CAP:
        return []
    return [
        f"{path}: {n} escalation questions in one round -- more than {ESCALATION_CAP} STUCK "
        f"rows at once is a harness signal, not {ESCALATION_CAP} scope decisions; open one "
        f"harness decide row instead (see plans/dispatch-self-healing-plan.md)"
    ]


def lint_memory(path):
    errs = []
    try:
        doc = yaml.safe_load(open(path))
    except Exception as e:  # noqa: BLE001 -- any parse failure is the finding
        return [f"{path}: does not parse: {e}"]
    if not isinstance(doc, dict):
        return [f"{path}: top level must be a mapping (version, cap, slots)"]
    if doc.get("version") != 1:
        errs.append(f"{path}: version must be 1")
    cap = doc.get("cap")
    if not (isinstance(cap, int) and 0 < cap <= MEMORY_CAP):
        errs.append(f"{path}: cap must be an integer between 1 and {MEMORY_CAP}")
        cap = MEMORY_CAP
    slots = doc.get("slots")
    if not isinstance(slots, list):
        return errs + [f"{path}: slots must be a list"]
    if len(slots) > cap:
        errs.append(f"{path}: {len(slots)} slots over cap {cap} -- drop a stale slot "
                    f"(one whose source no longer resolves) before adding one")
    seen = set()
    for i, s in enumerate(slots):
        where = f"{path}: slot {i} ({s.get('id', '?') if isinstance(s, dict) else '?'})"
        if not isinstance(s, dict):
            errs.append(f"{where}: not a mapping")
            continue
        kind = s.get("kind")
        if kind not in MEMORY_KINDS:
            errs.append(f"{where}: kind must be one of {sorted(MEMORY_KINDS)} -- incident "
                        f"or procedure narrative belongs in plans/simple-dispatch-design.md")
        # A watch is a condition plus what prompted it; every other kind is
        # a one-line summary of the fact itself.
        body = ("condition", "observation") if kind == "watch" else ("summary",)
        for field in ("id", "source", "written", *body):
            value = s.get(field)
            if not (isinstance(value, str) and value.strip()):
                errs.append(f"{where}: missing or empty '{field}'")
            elif field in ("condition", "summary") and "\n" in value.strip():
                errs.append(f"{where}: '{field}' must be one line")
        confirmed = s.get("confirmed")
        if not (isinstance(confirmed, int) and not isinstance(confirmed, bool)
                and confirmed >= 0):
            errs.append(f"{where}: confirmed must be a non-negative integer")
        if s.get("id") in seen:
            errs.append(f"{where}: duplicate id")
        seen.add(s.get("id"))
    return errs


def main():
    board_errs = lint(str(BOARD), archived=False)
    errs = board_errs + lint(str(ARCHIVE), archived=True)
    errs += lint_memory(str(MEMORY))
    # Only a schema-valid board is worth reading for staleness: the rows
    # above may not even be mappings.
    if not board_errs:
        errs += lint_stale_delegated(yaml.safe_load(open(BOARD)), DISPATCH_LOG)
    for round_path in sorted(GRILL_DIR.glob("*.round.yaml")):
        errs += lint_round_escalations(round_path)
    for e in errs:
        print(e, file=sys.stderr)
    sys.exit(1 if errs else 0)

if __name__ == "__main__":
    main()
