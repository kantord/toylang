#!/usr/bin/env python3
"""Ground truth for "is a sandboxed dispatch actually still working" --
independent of whether its msb VM still shows "running" in `msb list`.

A sandbox kept alive for post-mortem debugging after sandbox_dispatch.py has
already exited (landed, escalated, or anomaly-kept) is NOT still in progress,
but `msb list`'s "running" status can't tell the difference. Found live,
2026-09-06: this false-positive (a) silently exhausted the WIP-3 dispatch
cap by counting idle debug sandboxes as occupied slots, and (b) made
stuck-watch.py permanently blind to a stuck row sitting behind a kept
anomaly sandbox, since both read `msb list`'s status as ground truth.

The one true signal: sandbox_dispatch.py's own host process runs for the
ENTIRE dispatch lifetime (plan-decompose, build, verify, land) and only
exits once the cycle is fully resolved one way or another -- whatever
happens to its msb VM afterward is irrelevant to whether dispatch is still
active.

Usage:
  sandbox_dispatch_status.py                  # one issue-id per line, active dispatches
  sandbox_dispatch_status.py --count          # just the count
  sandbox_dispatch_status.py --gc             # remove orphaned kept sandboxes, report what was removed
  sandbox_dispatch_status.py --dispatch-trigger [--cap N]
      # bash-trigger text ("N free sandbox slot(s), ready: ...") or nothing,
      # for drive-tick.sh's zero-token skip check. Default cap 3 (kanban
      # ruling, 2026-09-06).
"""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

REPO = Path("/home/kantord/repos/toylang")
MSB_BIN = str(Path.home() / ".local/bin/msb")
DEFAULT_CAP = 3
GC_MIN_AGE_SECONDS = 15 * 60  # grace period for the rare manual --keep-sandbox case


def active_issue_ids() -> list[str]:
    """issue-ids of currently-running `sandbox_dispatch.py <issue-id> ...`
    processes on the host, read straight from /proc -- no msb involved."""
    ids = []
    for pid in os.listdir("/proc"):
        if not pid.isdigit():
            continue
        try:
            with open(f"/proc/{pid}/cmdline", "rb") as f:
                argv = [a.decode(errors="replace") for a in f.read().split(b"\0") if a]
        except OSError:
            continue
        try:
            i = next(i for i, a in enumerate(argv) if a.endswith("sandbox_dispatch.py"))
        except StopIteration:
            continue
        if i + 1 < len(argv):
            ids.append(argv[i + 1])
    return ids


def _sh(args: list[str]) -> tuple[int, str]:
    r = subprocess.run(args, capture_output=True, text=True, timeout=30)
    return r.returncode, r.stdout


def _msb_sandbox_rows() -> list[tuple[str, str]]:
    """[(name, created_str)] for every sd-* sandbox in `msb list`."""
    rc, out = _sh([MSB_BIN, "list"])
    if rc != 0:
        return []
    rows = []
    for line in out.splitlines()[1:]:  # header: NAME IMAGE STATUS CREATED
        parts = line.split(None, 3)
        if len(parts) >= 4 and parts[0].startswith("sd-"):
            rows.append((parts[0], parts[3]))
    return rows


def _open_escalation_stems() -> set[str]:
    grill_dir = REPO / "docs" / ".grill"
    if not grill_dir.is_dir():
        return set()
    suffix = "-sandbox-blocker.round.yaml"
    return {p.name[: -len(suffix)] for p in grill_dir.iterdir() if p.name.endswith(suffix)}


def _matches(issue_id: str, sandbox_suffix: str) -> bool:
    """The sandbox VM name is `sd-<issue_id>` truncated to 32 chars, so an
    exact match is only guaranteed for short issue-ids -- fall back to
    prefix matching in either direction, same convention stuck-watch.py
    already uses for this exact truncation."""
    return (issue_id == sandbox_suffix
            or issue_id.startswith(sandbox_suffix)
            or sandbox_suffix.startswith(issue_id))


def gc_orphaned_sandboxes() -> list[str]:
    """Remove a kept sandbox once it is PROVABLY done being useful: no
    active dispatch process for it, and no open escalation round either (an
    anomaly-keep always writes a round right before the process exits, so
    the round being gone means someone already consumed it). Age-gated as
    defense in depth for the one path that writes no round at all: a manual
    `--keep-sandbox` run that happened to land clean."""
    import time
    active = active_issue_ids()
    open_stems = _open_escalation_stems()
    removed = []
    for name, created in _msb_sandbox_rows():
        suffix = name[3:]
        if any(_matches(a, suffix) for a in active):
            continue
        if any(_matches(stem, suffix) for stem in open_stems):
            continue
        try:
            import datetime
            age = time.time() - datetime.datetime.strptime(created, "%Y-%m-%d %H:%M:%S").timestamp()
        except ValueError:
            age = GC_MIN_AGE_SECONDS  # unknown format -- don't block GC on it
        if age < GC_MIN_AGE_SECONDS:
            continue
        subprocess.run([MSB_BIN, "rm", "-f", name], capture_output=True)
        removed.append(name)
    return removed


def dispatch_trigger(cap: int = DEFAULT_CAP) -> str | None:
    import glob
    import os
    import yaml
    active = active_issue_ids()
    free = cap - len(active)
    if free <= 0:
        return None
    with open(REPO / "plans/board.yaml") as f:
        rows = yaml.safe_load(f) or []
    # A needs-id merely ABSENT from the live board is not necessarily done --
    # it can equally mean the id was never boarded at all (a typo or a
    # forgotten follow-up row), which is not "unblocked", just broken. Found
    # live, 2026-09-07: variant-types-flip's `needs: [matcher-totality-and-
    # alt-design, ...]` names a row that exists nowhere, live or archived --
    # the coordinator had to catch this by hand every tick because this
    # function's original "absent from live board" check (matching the
    # board's own documented issue-#113 shorthand) treated it as satisfied.
    # Board-archive.yaml is the actual, unambiguous record of "genuinely
    # landed" -- require presence there instead.
    with open(REPO / "plans/board-archive.yaml") as f:
        archived = yaml.safe_load(f) or []
    done = {r["id"] for r in archived}
    # A standing "do not redispatch" hold -- the dispatch-worker.sh-era
    # escalated-<lane> marker convention, still actively maintained by hand
    # (confirmed live, 2026-09-07: euler-slow-fragments-2's own title
    # documents exactly this, and ~/.cache/toylang-drive/escalated-issue-93
    # exists on disk). Dropped when this function replaced the old cap-8
    # dispatch trigger; restored, matched by the row's `issue: gh:N` field.
    escalated_issues = {
        os.path.basename(p)[len("escalated-issue-"):]
        for p in glob.glob(os.path.expanduser("~/.cache/toylang-drive/escalated-issue-*"))
    }

    def is_ready(r: dict) -> bool:
        if r.get("status") != "todo" or r.get("kind") != "build":
            return False
        # board.yaml's status field can lag a genuinely-dispatched row by a
        # tick or two (dispatch and the "mark delegated" commit are separate
        # steps) -- found live, 2026-09-07: two rows still read `status:
        # todo` while their own sandbox_dispatch.py process was actively
        # running (finishing land-lane.sh). Trust the live process list over
        # the field to avoid recommending a double-dispatch in that window.
        if r["id"] in active:
            return False
        gh = str(r.get("issue", ""))
        if gh.startswith("gh:") and gh[3:] in escalated_issues:
            return False
        return all(n in done for n in r.get("needs", []))

    ready = [r["id"] for r in rows if is_ready(r)]
    if not ready:
        return None
    return f"{free} free sandbox slot(s) (cap {cap}), ready: {' '.join(ready[:3])}"


if __name__ == "__main__":
    args = sys.argv[1:]
    if "--gc" in args:
        for n in gc_orphaned_sandboxes():
            print(f"removed orphaned sandbox: {n}")
    elif "--count" in args:
        print(len(active_issue_ids()))
    elif "--dispatch-trigger" in args:
        cap = DEFAULT_CAP
        if "--cap" in args:
            cap = int(args[args.index("--cap") + 1])
        t = dispatch_trigger(cap)
        if t:
            print(t)
    else:
        for i in active_issue_ids():
            print(i)
