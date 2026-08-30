#!/usr/bin/env bash
# The drive loop: one stateless tick, a pause, the next tick. Start it manually
# (a terminal or kitty window is fine); stop it by killing the process -- each
# tick is atomic, so a kill between ticks loses nothing, and mid-tick the flock
# in drive-tick.sh keeps a restarted loop from doubling up.
#
#   DRIVE_INTERVAL  seconds between ticks (default 600)
#   AUDIT_EVERY     every Nth tick runs the audit instead (default 30, ~5h)
set -uo pipefail
DIR="$(cd "$(dirname "$0")" && pwd)"
INTERVAL="${DRIVE_INTERVAL:-600}"
AUDIT_EVERY="${AUDIT_EVERY:-30}"

n=0
while true; do
  n=$((n + 1))
  if [ $((n % AUDIT_EVERY)) -eq 0 ]; then
    "$DIR/drive-tick.sh" audit
  else
    "$DIR/drive-tick.sh"
  fi
  echo "[drive-loop] tick $n done $(date '+%H:%M:%S'), next in ${INTERVAL}s"
  sleep "$INTERVAL"
done
