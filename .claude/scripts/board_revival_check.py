#!/usr/bin/env python3
"""Advisory check, not a gate: which parked rows might be dispatch-ready again.

`blocked_by` (see board-lint.py) and `status: proposed` both mean "not
dispatchable right now" -- but nothing ever re-checks whether the reason
still holds once the row's own `needs:` prerequisite actually lands. A row
that's blocked purely on an unmet `needs` id has a structural signal this
script CAN check automatically (is that id now status: done); a row whose
`blocked_by` reason has no corresponding `needs` id (e.g. "hand off to a
manual session," with no row to point at) has nothing this script can act
on -- that half of the gap still needs a human or a tick to reconsider it
periodically. This only covers the half a machine actually can.

Prints one line per candidate to stdout; empty output means nothing to
revisit. Always exits 0 -- this is a nudge for the tick/audit prompt to
read and judge, not a hard failure a writer should be blocked by.
"""

import sys
from pathlib import Path

import yaml

REPO = Path(__file__).resolve().parent.parent.parent


def _load(path: Path) -> list[dict]:
    try:
        rows = yaml.safe_load(open(path))
    except (FileNotFoundError, yaml.YAMLError):
        return []
    return rows if isinstance(rows, list) else []


def find_revival_candidates(rows: list[dict], all_status_by_id: dict[str, str | None]) -> list[str]:
    """Rows that are parked (proposed, or todo+blocked_by would be a lint
    error so that combination shouldn't exist -- but proposed or any
    non-todo/non-done status with blocked_by set both count) whose every
    `needs` id now resolves to status: done elsewhere on the board."""
    findings = []
    for r in rows:
        if not isinstance(r, dict):
            continue
        if r.get("status") not in ("proposed",) and not r.get("blocked_by"):
            continue
        needs = r.get("needs") or []
        if not needs:
            continue  # nothing structural to re-check; the blocker (if any) needs a human
        if all(all_status_by_id.get(n) == "done" for n in needs):
            if r.get("blocked_by"):
                # `needs` being done is necessary but not sufficient here -- the row itself
                # says the real gate is `blocked_by`'s reason, which `needs` may only
                # partially or coincidentally represent (confirmed on two real rows,
                # 2026-09-13: one's `needs` pointed at a design-ratified row when the actual
                # gate was the corresponding BUILD landing, with no row of its own yet; the
                # other's `needs` was essentially unrelated to a maintainer-ruling hold).
                # Read blocked_by before reviving, don't just flip status on this line alone.
                findings.append(
                    f"{r['id']}: status={r.get('status')}, needs {needs} all done, but "
                    f"blocked_by says: {r['blocked_by']!r} -- read that before reviving, "
                    f"needs-done alone may not mean the real gate cleared"
                )
            else:
                findings.append(
                    f"{r['id']}: status={r.get('status')}, needs {needs} all done -- "
                    f"reconsider reviving to todo"
                )
    return findings


def main() -> None:
    board = _load(REPO / "plans" / "board.yaml")
    archive = _load(REPO / "plans" / "board-archive.yaml")
    all_status_by_id = {r["id"]: r.get("status") for r in board + archive if isinstance(r, dict) and "id" in r}

    for line in find_revival_candidates(board, all_status_by_id):
        print(line)
    sys.exit(0)


if __name__ == "__main__":
    main()
