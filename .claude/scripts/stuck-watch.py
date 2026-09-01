#!/usr/bin/env python3
"""Deterministic stuck-lane watchdog (maintainer design, 2026-09-01).

Runs from drive-tick.sh's bash section on every tick, models nothing, asks
nothing. Three jobs:

1. HISTORY: append one timestamped JSON line per lane to lane-history.jsonl --
   ahead/dirty/live/runs/last_activity -- so "how long has this been stuck"
   is a query over a ledger instead of an agent's guess.
2. DETECT: a lane is stuck when it has no live worker, no commits ahead, at
   least one attempted run, and no activity for STUCK_AFTER seconds.
3. CONVERT: snapshot the evidence (last event log tail, git status/diff) into
   plans/incidents/ BEFORE a redispatch can destroy it, insert a top-priority
   investigation row into plans/board.yaml, and commit both. The row asks WHY
   the lane got stuck (brief clarity / capability gap / tooling trap / task
   shape), not for the task itself to be done.

Dedup: the board row id (stuck-issue-N-investigation) is checked against both
board files; the investigating-issue-N marker survives until the lane lands
(land-lane.sh clears it), so one stuck episode yields one investigation.
"""

import glob
import json
import os
import subprocess
import sys
import time

REPO = "/home/kantord/repos/toylang"
LANES = os.path.expanduser("~/.local/share/toylang-lanes")
LOG_DIR = os.path.expanduser("~/.cache/toylang-drive")
OC_DIR = os.path.join(LOG_DIR, "opencode")
HISTORY = os.path.join(LOG_DIR, "lane-history.jsonl")
STUCK_AFTER = 6 * 3600

def sh(args, cwd=None):
    r = subprocess.run(args, cwd=cwd, capture_output=True, text=True, timeout=60)
    return r.returncode, r.stdout

def live_worker_dirs():
    dirs = set()
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            comm = open(f"/proc/{pid}/comm").read().strip()
            if comm not in ("opencode", "claude"):
                continue
            dirs.add(os.readlink(f"/proc/{pid}/cwd"))
        except OSError:
            continue
    return dirs

def lane_state(d, live_dirs):
    name = os.path.basename(d.rstrip("/"))
    real = os.path.realpath(d)
    _, ahead = sh(["git", "-C", d, "rev-list", "--count", "main..HEAD"])
    _, status = sh(["git", "-C", d, "status", "--porcelain"])
    tracked = [l for l in status.splitlines() if not l.startswith("??")]
    logs = sorted(glob.glob(os.path.join(OC_DIR, f"*-{name}.jsonl")))
    last_log = max((os.path.getmtime(p) for p in logs), default=0)
    _, ct = sh(["git", "-C", d, "log", "-1", "--format=%ct"])
    return {
        "ts": int(time.time()),
        "lane": name,
        "ahead": int(ahead.strip() or 0),
        "tracked_dirty": len(tracked),
        "live": any(w.startswith(real) for w in live_dirs),
        "runs": len(logs),
        "last_activity": int(max(last_log, float(ct.strip() or 0))),
    }

def board_has_row(row_id):
    for f in ("plans/board.yaml", "plans/board-archive.yaml"):
        with open(os.path.join(REPO, f)) as fh:
            if row_id in fh.read():
                return True
    return False

def snapshot_evidence(lane, st):
    day = time.strftime("%Y%m%d")
    inc = os.path.join(REPO, "plans", "incidents", f"{lane}-{day}")
    os.makedirs(inc, exist_ok=True)
    d = os.path.join(LANES, lane)
    _, status = sh(["git", "-C", d, "status", "--porcelain"])
    _, diffstat = sh(["git", "-C", d, "diff", "--stat"])
    with open(os.path.join(inc, "worktree-state.txt"), "w") as f:
        f.write(f"captured: {time.strftime('%Y-%m-%dT%H:%M:%S%z')}\n"
                f"state: {json.dumps(st)}\n\n=== git status ===\n{status}"
                f"\n=== git diff --stat ===\n{diffstat}")
    logs = sorted(glob.glob(os.path.join(OC_DIR, f"*-{lane}.jsonl")))
    for p in logs[-2:]:
        lines = open(p, errors="replace").readlines()
        with open(os.path.join(inc, os.path.basename(p) + ".tail"), "w") as f:
            f.writelines(lines[-300:])
    return os.path.relpath(inc, REPO)

def insert_row(lane, inc_rel, st):
    n = lane.split("-", 1)[1]
    hours = (int(time.time()) - st["last_activity"]) // 3600
    row = (
        f"- id: stuck-{lane}-investigation\n"
        f"  kind: build\n"
        f"  status: todo\n"
        f"  needs: []\n"
        f"  issue: gh:{n}\n"
        f"  title: 'INVESTIGATE stuck lane {lane} (no activity {hours}h, {st['runs']}"
        f" run(s), 0 commits): evidence frozen in {inc_rel}/ -- read it plus the lane"
        f" worktree, then report in plans/opencode-rollout.md whether this was brief"
        f" clarity, a capability gap, a tooling/permission trap, or task shape, and"
        f" propose the rebrief or reshape. Do NOT attempt the original task.'\n"
    )
    path = os.path.join(REPO, "plans/board.yaml")
    text = open(path).read()
    i = text.find("- id:")
    if i < 0:
        raise RuntimeError("no rows in board.yaml")
    new = text[:i] + row + text[i:]
    import yaml
    yaml.safe_load(new)  # refuse to write a board the site cannot parse
    open(path, "w").write(new)

def commit(lane, inc_rel):
    sh(["git", "-C", REPO, "add", "plans/board.yaml", inc_rel])
    for _ in range(6):
        rc, _ = sh(["git", "-C", REPO, "commit",
                    "-m", f"board: auto-file stuck-lane investigation for {lane}\n\n"
                          f"Filed by stuck-watch.py (deterministic watchdog): evidence\n"
                          f"frozen in {inc_rel}/ at detection time, row inserted at top\n"
                          f"priority so the diagnosis runs before a redispatch destroys\n"
                          f"the worktree state.\n\n"
                          f"Co-Authored-By: stuck-watch.py (deterministic)",
                    "--", "plans/board.yaml", f"{inc_rel}"])
        if rc == 0:
            return True
        time.sleep(5)
    return False

def main():
    os.makedirs(OC_DIR, exist_ok=True)
    live_dirs = live_worker_dirs()
    now = int(time.time())
    with open(HISTORY, "a") as hist:
        for d in sorted(glob.glob(os.path.join(LANES, "issue-*/"))):
            lane = os.path.basename(d.rstrip("/"))
            if not lane.replace("issue-", "").isdigit():
                continue
            st = lane_state(d, live_dirs)
            hist.write(json.dumps(st) + "\n")
            marker = os.path.join(LOG_DIR, f"investigating-{lane}")
            # A lane parked on a maintainer escalation is stuck ON PURPOSE --
            # it is on the maintainer's desk, not lost; filing an investigation
            # would duplicate the escalation (issue-93's privileged handoff).
            if os.path.exists(os.path.join(LOG_DIR, f"escalated-{lane}")):
                continue
            board_busy = sh(["git", "-C", REPO, "diff", "--quiet",
                             "--", "plans/board.yaml"])[0] != 0
            if (not st["live"] and st["ahead"] == 0 and st["runs"] >= 1
                    and now - st["last_activity"] >= STUCK_AFTER
                    and not os.path.exists(marker)
                    and not board_busy  # a tick mid-edit owns the board; retry next tick
                    and not board_has_row(f"stuck-{lane}-investigation")):
                inc_rel = snapshot_evidence(lane, st)
                insert_row(lane, inc_rel, st)
                if commit(lane, inc_rel):
                    open(marker, "w").write(str(now))
                    print(f"[stuck-watch] filed investigation for {lane} ({inc_rel})")
                else:
                    print(f"[stuck-watch] {lane}: commit failed, will retry next tick",
                          file=sys.stderr)

if __name__ == "__main__":
    main()
