#!/usr/bin/env python3
"""SessionEnd hook: append one CSV row of usage telemetry per ended session.

Zero model tokens -- the session records itself as it exits (gh:123). Reads the
hook's stdin JSON for the transcript path, walks the transcript once, and appends
to ~/.cache/toylang-drive/lanes.csv:

    ended_at, kind, lane, session_id, model, turns, output_tokens,
    peak_context, wall_seconds

kind is worker (an enwiro worktree), tick (the repo checkout), or other. Never
fails the session: any error exits 0 silently.
"""
import csv
import json
import os
import re
import sys
from datetime import datetime, timezone

try:
    hook = json.load(sys.stdin)
    transcript = hook.get("transcript_path") or ""
    cwd = hook.get("cwd") or ""
    sid = (hook.get("session_id") or "")[:8]
    if not transcript or not os.path.exists(transcript):
        sys.exit(0)

    turns = out_tok = peak_ctx = 0
    model = ""
    first_ts = last_ts = None
    for line in open(transcript):
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        ts = e.get("timestamp")
        if ts:
            first_ts = first_ts or ts
            last_ts = ts
        if e.get("type") != "assistant":
            continue
        msg = e.get("message", {})
        model = msg.get("model") or model
        u = msg.get("usage")
        if u:
            turns += 1
            out_tok += u.get("output_tokens", 0)
            ctx = sum(u.get(k, 0) for k in
                      ("input_tokens", "cache_read_input_tokens",
                       "cache_creation_input_tokens"))
            peak_ctx = max(peak_ctx, ctx)
    if turns == 0:
        sys.exit(0)

    wall = 0
    if first_ts and last_ts:
        try:
            p = lambda t: datetime.fromisoformat(t.replace("Z", "+00:00"))
            wall = int((p(last_ts) - p(first_ts)).total_seconds())
        except ValueError:
            pass

    if "/enwiro/worktrees/" in cwd:
        # Pool lane worktrees (gh:124) are cooked as <lane>-<8 hex>; strip the
        # suffix so rows for one lane share a name across recycles.
        kind = "worker"
        lane = re.sub(r"-[0-9a-f]{8}$", "", os.path.basename(cwd))
    elif cwd.rstrip("/").endswith("repos/toylang"):
        kind, lane = "tick", "coordinator"
    else:
        kind, lane = "other", os.path.basename(cwd)

    out = os.path.expanduser("~/.cache/toylang-drive/lanes.csv")
    os.makedirs(os.path.dirname(out), exist_ok=True)
    new = not os.path.exists(out)
    with open(out, "a", newline="") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["ended_at", "kind", "lane", "session_id", "model",
                        "turns", "output_tokens", "peak_context", "wall_seconds"])
        w.writerow([datetime.now(timezone.utc).isoformat(timespec="seconds"),
                    kind, lane, sid, model, turns, out_tok, peak_ctx, wall])
except Exception:
    pass
sys.exit(0)
