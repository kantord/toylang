#!/usr/bin/env bash
# Watch what the current coordinator tick is doing, live: tail its session
# transcript through the same colorizer the loop terminal uses. Ctrl-C to stop
# (detaches the viewer only; the tick is untouched).
set -uo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
T=$(ls -t "$HOME"/.claude/projects/-home-kantord-repos-toylang/*.jsonl | head -1)
echo "peeking: $T"
tail -n 40 -f "$T" | python3 "$DIR/tick-stream.py" /dev/null
