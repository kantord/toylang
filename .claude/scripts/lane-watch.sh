#!/usr/bin/env bash
# Watches the delegated lanes and EXITS the moment one looks finished, so the coordinator's
# harness notification replaces quiet-period polling (which cost 30-45 minutes of landing
# latency per lane before this existed). Finished signature: at least one commit past main,
# clean tree, and no file writes for QUIET_SECS. Reads the delegated set fresh from
# plans/board.yaml every cycle, so a lane dispatched after launch is watched too.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WT_BASE="$HOME/.local/share/enwiro/worktrees/pr/toylang-1234138d"
QUIET_SECS="${QUIET_SECS:-480}"

while true; do
  lanes=$(python3 -c "
import yaml
b=yaml.safe_load(open('$ROOT/plans/board.yaml'))
for r in b:
    if r['status']=='delegated' and r.get('issue'):
        print(str(r['issue']).split(':')[1])" 2>/dev/null)
  [ -z "$lanes" ] && { echo "no delegated lanes remain"; exit 0; }
  now=$(date +%s)
  for n in $lanes; do
    wt="$WT_BASE/issue-$n"
    [ -d "$wt" ] || continue
    commits=$(git -C "$wt" log --oneline "main..issue-$n" 2>/dev/null | wc -l)
    dirty=$(git -C "$wt" status --short 2>/dev/null | wc -l)
    [ "$commits" -gt 0 ] && [ "$dirty" -eq 0 ] || continue
    last_commit=$(git -C "$wt" log -1 --format=%ct 2>/dev/null || echo "$now")
    # newest non-git write in the worktree, in case the worker edits without committing yet
    newest=$(find "$wt" -newer "$wt/.git/HEAD" -type f -not -path '*/.git/*' \
      -not -path '*/target/*' -not -path '*/node_modules/*' -printf '%T@\n' 2>/dev/null | sort -rn | head -1)
    newest=${newest%%.*}; newest=${newest:-$last_commit}
    ref=$(( last_commit > newest ? last_commit : newest ))
    if [ $(( now - ref )) -ge "$QUIET_SECS" ]; then
      echo "LANE FINISHED: issue-$n (quiet $(( (now - ref) / 60 ))m, $commits commit(s))"
      exit 0
    fi
  done
  sleep 60
done
