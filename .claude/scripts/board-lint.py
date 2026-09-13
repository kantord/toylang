#!/usr/bin/env python3
"""Schema validator for plans/board.yaml and plans/board-archive.yaml.

The board has several deterministic writers now (ticks, board-archive.py,
humans), and the site renders whatever they wrote: a row
missing a field the UI dereferences blanked the whole kanban to empty
(needs-less rows, 2026-09-01). Parsing is not validity -- this is the schema
gate, run by every writer before committing and by .claude/checks at Stop.

`blocked_by` (2026-09-13): a row can be genuinely not-dispatchable for a
reason `needs:` can't express -- no other row id to point at (an unbuilt
mechanism with no row of its own yet), a maintainer ruling to hand off
manually, or similar. Before this field existed the only place that fact
lived was the row's own free-text title, which `is_ready()` in
dispatch_state.py never reads: dsv-partials-migration and
euler-slow-fragments-2 both sat at status: todo (mechanically "ready",
needs satisfied) for 6+ ticks across several hours, each one independently
re-reading the title, agreeing it's blocked, and skipping -- without ever
fixing the status field, so the next tick paid the same cost again. Setting
`blocked_by` makes that fact structural instead of prose a human has to
notice: if it's set, status must not be `todo` (enforced below), so a row
can't silently sit "ready" while also declaring itself not ready.

Exit 0 = valid. Exit 1 = findings on stderr, one per line.
"""

import sys

import yaml

KINDS = {"build", "decide"}
STATUSES = {"todo", "delegated", "done", "proposed"}

def lint(path, archived):
    errs = []
    try:
        rows = yaml.safe_load(open(path))
    except Exception as e:  # noqa: BLE001 -- any parse failure is the finding
        return [f"{path}: does not parse: {e}"]
    if not isinstance(rows, list):
        return [f"{path}: top level must be a list of rows"]
    seen = set()
    for i, r in enumerate(rows):
        where = f"{path}: row {i} ({r.get('id', '?') if isinstance(r, dict) else '?'})"
        if not isinstance(r, dict):
            errs.append(f"{where}: not a mapping")
            continue
        for field in ("id", "title"):
            if not isinstance(r.get(field), str) or not r.get(field).strip():
                errs.append(f"{where}: missing or empty '{field}'")
        if r.get("kind") not in KINDS:
            errs.append(f"{where}: kind must be one of {sorted(KINDS)}")
        if r.get("status") not in STATUSES:
            errs.append(f"{where}: status must be one of {sorted(STATUSES)}")
        if archived and r.get("status") != "done":
            errs.append(f"{where}: archive rows must be status: done")
        needs = r.get("needs", [])
        if not (isinstance(needs, list)
                and all(isinstance(n, str) for n in needs)):
            errs.append(f"{where}: needs must be a list of row ids")
        issue = r.get("issue")
        if issue is not None and not (isinstance(issue, str)
                                      and issue.startswith("gh:")):
            errs.append(f"{where}: issue must look like gh:<number>")
        blocked_by = r.get("blocked_by")
        if blocked_by is not None:
            if not (isinstance(blocked_by, str) and blocked_by.strip()):
                errs.append(f"{where}: blocked_by must be a non-empty string when present")
            if r.get("status") == "todo":
                errs.append(
                    f"{where}: status is todo but blocked_by is set -- "
                    f"a row can't be both dispatch-ready and declare itself blocked; "
                    f"set status to proposed or clear blocked_by"
                )
        if r["id"] in seen:
            errs.append(f"{where}: duplicate id")
        seen.add(r.get("id"))
    return errs

def main():
    errs = lint("plans/board.yaml", archived=False)
    errs += lint("plans/board-archive.yaml", archived=True)
    for e in errs:
        print(e, file=sys.stderr)
    sys.exit(1 if errs else 0)

if __name__ == "__main__":
    main()
