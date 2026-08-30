#!/usr/bin/env python3
"""Report a worker lane's last session id, model, and context size (gh:124).

The coordinator runs this at dispatch time to decide reuse vs recycle: a lane
whose context is under the ceiling takes the next task in the same session
(SendMessage to a live worker, claude --resume for a dead one); over the
ceiling, or unmeasurable, the lane gets a fresh session in the same worktree.

    lane-context.py <worktree-path> [--log-dispatch gh:N]

Prints: sid=<id> model=<model> context=<tokens> source=<transcript|telemetry>
Exits 1 (silently) when the worktree has no session at all -- a cold lane.

Context is measured from whichever of these exists, in order:
  1. the session's transcript (~/.claude/projects/<munged-cwd>/<sid>.jsonl),
     summing the LAST assistant usage entry's input+cache tokens -- what the
     most recent request actually carried, the same measure drive-tick.sh
     watches for the coordinator session. Interactive sessions on the newer
     harness do not expose a transcript, hence:
  2. the lane's last lanes.csv row (the gh:123 SessionEnd hook): peak_context
     at the previous session's end. Only present once a session has ended.
Neither existing prints context=unknown; the skill says unknown means recycle.

--log-dispatch appends a row to ~/.cache/toylang-drive/dispatches.csv so the
per-task cost of a reused lane stays recoverable (lanes.csv aggregates a whole
session, which under the pool spans tasks), and so the coordinator can count
tasks-this-session -- the recycle backstop when context is unmeasurable.
"""
import csv
import glob
import json
import os
import re
import sys
from datetime import datetime, timezone

LANES_CSV = os.path.expanduser("~/.cache/toylang-drive/lanes.csv")
DISPATCHES_CSV = os.path.expanduser("~/.cache/toylang-drive/dispatches.csv")


def lane_name(worktree):
    # Pool worktrees are cooked as <branch>-<8 hex>; strip the suffix so the
    # name matches lanes.csv (lane-telemetry.py strips it the same way).
    base = os.path.basename(os.path.realpath(worktree))
    return re.sub(r"-[0-9a-f]{8}$", "", base)


def project_dirs(worktree):
    # Claude munges the session cwd into the project dir name. The cwd may be
    # the enwiro symlink or the resolved worktree, so try both munges.
    paths = {os.path.abspath(worktree), os.path.realpath(worktree)}
    dirs = []
    for p in paths:
        munged = re.sub(r"[^A-Za-z0-9]", "-", p)
        d = os.path.expanduser(f"~/.claude/projects/{munged}")
        if os.path.isdir(d):
            dirs.append(d)
    return dirs


def last_session(worktree):
    """Newest session in the lane's project dir(s). Transcript-bearing
    sessions win over bare session dirs of the same age class, because an
    idle auto-spawned session (a workspace visit is enough to create one)
    leaves a dir with no real turns."""
    candidates = []  # (has_transcript, mtime, sid)
    for d in project_dirs(worktree):
        for t in glob.glob(os.path.join(d, "*.jsonl")):
            sid = os.path.splitext(os.path.basename(t))[0]
            candidates.append((1, os.path.getmtime(t), sid))
        for s in glob.glob(os.path.join(d, "*-*-*-*-*")):
            if os.path.isdir(s):
                candidates.append((0, os.path.getmtime(s), os.path.basename(s)))
    if not candidates:
        return None
    candidates.sort(reverse=True)
    return candidates[0][2]


def context_from_transcript(sid):
    for path in glob.glob(os.path.expanduser(f"~/.claude/projects/*/{sid}.jsonl")):
        model, ctx = "", 0
        for line in open(path):
            try:
                e = json.loads(line)
            except json.JSONDecodeError:
                continue
            if e.get("type") != "assistant":
                continue
            msg = e.get("message", {})
            model = msg.get("model") or model
            u = msg.get("usage")
            if u:
                ctx = sum(u.get(k, 0) for k in
                          ("input_tokens", "cache_read_input_tokens",
                           "cache_creation_input_tokens"))
        if ctx:
            return model, ctx
    return None


def context_from_telemetry(lane):
    if not os.path.exists(LANES_CSV):
        return None
    last = None
    for row in csv.DictReader(open(LANES_CSV)):
        if row.get("lane") == lane:
            last = row
    if last:
        return last.get("model", ""), int(last.get("peak_context") or 0)
    return None


def main():
    args = sys.argv[1:]
    issue = ""
    if "--log-dispatch" in args:
        i = args.index("--log-dispatch")
        issue = args[i + 1]
        del args[i:i + 2]
    if len(args) != 1:
        sys.exit(f"usage: {sys.argv[0]} <worktree-path> [--log-dispatch gh:N]")
    worktree = args[0]
    lane = lane_name(worktree)

    sid = last_session(worktree)
    if not sid:
        sys.exit(1)

    found = context_from_transcript(sid)
    source = "transcript"
    if not found:
        found = context_from_telemetry(lane)
        source = "telemetry"
    model, ctx = found if found else ("unknown", "unknown")
    if not found:
        source = "none"
    print(f"sid={sid} model={model} context={ctx} source={source}")

    if issue:
        os.makedirs(os.path.dirname(DISPATCHES_CSV), exist_ok=True)
        new = not os.path.exists(DISPATCHES_CSV)
        with open(DISPATCHES_CSV, "a", newline="") as f:
            w = csv.writer(f)
            if new:
                w.writerow(["dispatched_at", "lane", "issue", "session_id",
                            "model", "context_before"])
            w.writerow([datetime.now(timezone.utc).isoformat(timespec="seconds"),
                        lane, issue, sid[:8], model, ctx])


if __name__ == "__main__":
    main()
