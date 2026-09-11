#!/usr/bin/env python3
"""Delegated-worker launcher: opencode + a cheap OpenRouter model (maintainer ruling,
2026-08-30: claude-code delegation is retired; re-evaluate after ~30 landed lanes).

Runs from the lane worktree (enw wrap sets cwd) with the brief as argv[1]. NEVER
passes --auto: the guardrail is the mined allow-list permission config in the
maintainer's opencode.jsonc (deny-by-default; headless ask auto-refuses with
feedback the model adapts to). Captures the --format json event stream to a
per-run log, renders it live through opencode_peek, and appends a lanes.csv
telemetry row on exit so opencode lanes land in the same ledger the claude
SessionEnd hook feeds.
"""
import csv
import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import opencode_peek

SCRIPTS = Path(__file__).resolve().parent
DRIVE_LOG_DIR = Path.home() / ".cache" / "toylang-drive"
OPENCODE_LOG_DIR = DRIVE_LOG_DIR / "opencode"


def fire_next(lane: str) -> None:
    # Every lane goes straight to the serial landing queue (deterministic, no
    # model in the happy path; maintainer redesign 2026-09-01) -- land_lane.py
    # checks landability itself (a lane with nothing ahead of main, or one
    # whose gate goes red, no-ops or re-dispatches safely on its own),
    # handles conflict/red re-dispatch, and fires the tick when it is done.
    # cwd matters: a nohup child keeping cwd in this worktree would block its
    # removal, so this spawns from "/" like the bash version did.
    print(f"[opencode-worker] firing landing: {lane}")
    log_path = DRIVE_LOG_DIR / "land.log"
    with open(log_path, "a") as log:
        subprocess.Popen(
            [sys.executable, str(SCRIPTS / "land_lane.py"), "land", lane.removeprefix("issue-")],
            cwd="/", stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
            start_new_session=True,
        )


def run_worker(model: str, brief: str, lane: str, log_path: Path) -> int:
    env = dict(os.environ)
    env["PATH"] = str(Path.home() / ".cargo" / "bin") + os.pathsep + env.get("PATH", "")
    if shutil.which("sccache", path=env["PATH"]):
        env["RUSTC_WRAPPER"] = "sccache"

    errors_log = OPENCODE_LOG_DIR / "errors.log"
    # timeout --kill-after=30s 3600s: belt-and-suspenders against ANY
    # indefinite hang (opencode's `run` unconditionally awaits stdin EOF
    # whenever stdin isn't a TTY, with no timeout of its own -- confirmed via
    # strace + pty/no-pty A/B testing, 2026-09-05; stdin=DEVNULL below gives
    # it instant EOF, and this timeout is the same belt-and-suspenders
    # principle as drive_tick.py's own claude -p bound). 3600s vs the
    # coordinator's 2700s: the longest real worker run on record is 3255s
    # (lanes.csv), so give it headroom.
    with open(errors_log, "a") as errf:
        proc = subprocess.Popen(
            ["timeout", "--kill-after=30s", "3600s",
             "opencode", "run", "-m", model, "--format", "json", brief],
            stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=errf,
            text=True, env=env, start_new_session=True,
        )
        with open(log_path, "w") as log:
            for line in proc.stdout:
                log.write(line)
                log.flush()
                opencode_peek.process_line(line)
    proc.wait()
    return proc.returncode


def append_telemetry(log_path: Path, lane: str, start: int, end: int) -> None:
    sid = model = ""
    steps = out_tok = peak_ctx = 0
    cost = 0.0
    for line in open(log_path):
        try:
            e = json.loads(line)
        except json.JSONDecodeError:
            continue
        sid = sid or (e.get("sessionID") or "")[:12]
        p = e.get("part") or {}
        if e.get("type") == "step_finish":
            t = p.get("tokens") or {}
            steps += 1
            out_tok += t.get("output", 0)
            cache = t.get("cache") or {}
            peak_ctx = max(peak_ctx, t.get("input", 0) + cache.get("read", 0))
        if "cost" in p:
            cost += p.get("cost") or 0
        model = p.get("modelID") or model
    if steps == 0:
        return
    out = Path.home() / ".cache" / "toylang-drive" / "lanes.csv"
    new = not out.exists()
    with open(out, "a", newline="") as f:
        w = csv.writer(f)
        if new:
            w.writerow(["ended_at", "kind", "lane", "session_id", "model",
                        "turns", "output_tokens", "peak_context", "wall_seconds"])
        w.writerow([datetime.now(timezone.utc).isoformat(timespec="seconds"),
                    "worker", lane, sid, model or "deepseek-v4-flash-0731",
                    steps, out_tok, peak_ctx, end - start])
    print(f"[opencode-worker] done: {steps} steps, ${cost:.4f}, telemetry row appended")


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: opencode_worker.py '<kickoff brief>'", file=sys.stderr)
        return 2
    brief = sys.argv[1]
    model = os.environ.get("OPENCODE_MODEL", "openrouter/deepseek/deepseek-v4-flash-0731")
    OPENCODE_LOG_DIR.mkdir(parents=True, exist_ok=True)
    lane = Path.cwd().name
    ts = time.strftime("%Y%m%d-%H%M%S")
    log_path = OPENCODE_LOG_DIR / f"{ts}-{lane}.jsonl"

    print(f"[opencode-worker] {lane} on {model} (events: {log_path})")
    start = int(time.time())
    try:
        rc = run_worker(model, brief, lane, log_path)
    finally:
        # A wrapper death mid-run must still land the hand-off (one
        # process-group kill ate a worker AND its tick on 2026-08-30,
        # leaving the maintainer's answers waiting on the 600s loop
        # backstop) -- this finally block is the equivalent of the bash
        # version's `trap fire_next EXIT`, which fires on every exit path
        # except SIGKILL (the loop tick remains the backstop for that).
        fire_next(lane)
    end = int(time.time())
    append_telemetry(log_path, lane, start, end)
    return rc


if __name__ == "__main__":
    sys.exit(main())
