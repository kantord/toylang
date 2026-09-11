#!/usr/bin/env python3
"""The drive loop: one stateless tick, a pause, the next tick. Start it manually
(a terminal or kitty window is fine); stop it by killing the process -- each
tick is atomic, so a kill between ticks loses nothing, and mid-tick the flock
in drive_tick.py keeps a restarted loop from doubling up.

Env vars:
  DRIVE_INTERVAL  seconds between ticks (default 600)
  AUDIT_EVERY     every Nth tick runs the audit instead (default 30, ~5h)
"""
import os
import subprocess
import sys
import time
from datetime import datetime
from pathlib import Path

DIR = Path(__file__).resolve().parent


def main() -> None:
    interval = int(os.environ.get("DRIVE_INTERVAL", "600"))
    audit_every = int(os.environ.get("AUDIT_EVERY", "30"))
    n = 0
    while True:
        n += 1
        # sys.executable, not "uv run --project" again: this process is
        # already running inside the uv-resolved venv (it was itself started
        # via `uv run --project .claude/scripts drive_loop.py`), so its own
        # interpreter path already IS the locked environment -- re-invoking
        # `uv run` here would just pay ~35-48ms of redundant resolution
        # overhead per tick for the same result.
        args = [sys.executable, str(DIR / "drive_tick.py")]
        if n % audit_every == 0:
            args.append("audit")
        subprocess.run(args)
        print(f"[drive-loop] tick {n} done {datetime.now():%H:%M:%S}, next in {interval}s")
        time.sleep(interval)


if __name__ == "__main__":
    main()
