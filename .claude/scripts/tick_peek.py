#!/usr/bin/env python3
"""Watch what the current coordinator tick is doing, live: tail its session
transcript through the same colorizer the loop terminal uses. Ctrl-C to stop
(detaches the viewer only; the tick is untouched)."""
import time
from pathlib import Path

import tick_stream

SESSIONS_DIR = Path.home() / ".claude" / "projects" / "-home-kantord-repos-toylang"


def tail_forever(path: Path, initial_lines: int = 40):
    with open(path) as f:
        lines = f.readlines()
        for line in lines[-initial_lines:]:
            yield line
        while True:
            line = f.readline()
            if line:
                yield line
            else:
                time.sleep(0.5)


def main() -> None:
    candidates = sorted(SESSIONS_DIR.glob("*.jsonl"), key=lambda p: p.stat().st_mtime)
    if not candidates:
        raise SystemExit(f"no session transcripts found in {SESSIONS_DIR}")
    transcript = candidates[-1]
    print(f"peeking: {transcript}")
    for line in tail_forever(transcript):
        tick_stream.process_line(line, "/dev/null")


if __name__ == "__main__":
    main()
