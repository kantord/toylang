#!/usr/bin/env bash
# Delegated-worker launcher: opencode + a cheap OpenRouter model (maintainer ruling,
# 2026-08-30: claude-code delegation is retired; re-evaluate after ~30 landed lanes).
#
# Runs from the lane worktree (enw wrap sets cwd) with the brief as $1. NEVER passes
# --auto: the guardrail is the mined allow-list permission config in the maintainer's
# opencode.jsonc (deny-by-default; headless ask auto-refuses with feedback the model
# adapts to). Captures the --format json event stream to a per-run log, renders it
# live through opencode-peek.py, and appends a lanes.csv telemetry row on exit so
# opencode lanes land in the same ledger the claude SessionEnd hook feeds.
set -uo pipefail
MODEL="${OPENCODE_MODEL:-openrouter/deepseek/deepseek-v4-flash-0731}"
BRIEF="${1:?usage: opencode-worker.sh '<kickoff brief>'}"
LOG_DIR="$HOME/.cache/toylang-drive/opencode"
mkdir -p "$LOG_DIR"
LANE=$(basename "$PWD")
TS=$(date +%Y%m%d-%H%M%S)
LOG="$LOG_DIR/$TS-$LANE.jsonl"
SCRIPTS="$(cd "$(dirname "$0")" && pwd)"

echo "[opencode-worker] $LANE on $MODEL (events: $LOG)"
START=$(date +%s)
opencode run -m "$MODEL" --format json "$BRIEF" 2>>"$LOG_DIR/errors.log" \
  | tee "$LOG" | python3 "$SCRIPTS/opencode-peek.py"
RC=$?
END=$(date +%s)

python3 - "$LOG" "$LANE" "$START" "$END" <<'EOF'
import csv, json, os, sys
from datetime import datetime, timezone
log, lane, start, end = sys.argv[1], sys.argv[2], int(sys.argv[3]), int(sys.argv[4])
sid = model = ""
steps = out_tok = peak_ctx = 0
cost = 0.0
for line in open(log):
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
    sys.exit(0)
out = os.path.expanduser("~/.cache/toylang-drive/lanes.csv")
new = not os.path.exists(out)
with open(out, "a", newline="") as f:
    w = csv.writer(f)
    if new:
        w.writerow(["ended_at", "kind", "lane", "session_id", "model",
                    "turns", "output_tokens", "peak_context", "wall_seconds"])
    w.writerow([datetime.now(timezone.utc).isoformat(timespec="seconds"),
                "worker", lane, sid, model or "deepseek-v4-flash-0731",
                steps, out_tok, peak_ctx, end - start])
print(f"[opencode-worker] done: {steps} steps, ${cost:.4f}, telemetry row appended")
EOF
exit $RC
