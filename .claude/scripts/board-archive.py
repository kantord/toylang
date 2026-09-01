#!/usr/bin/env python3
"""Move board rows to the archive, terminator-anchored (issue #113 flow, scripted).

Usage: board-archive.py <row-id> [<row-id>...]

Cuts each `- id: <slug>` block (with any comment lines directly above it) out of
plans/board.yaml, appends it to plans/board-archive.yaml with status flipped to done,
validates both files, and prints what moved. Exits nonzero -- changing nothing -- on any
miss or validation failure. The caller commits.
"""
import sys

import yaml

BOARD = "plans/board.yaml"
ARCHIVE = "plans/board-archive.yaml"


def cut_row(text: str, row_id: str) -> tuple[str, str]:
    anchor = f"- id: {row_id}\n"
    i = text.find(anchor)
    if i < 0 or (i > 0 and text[i - 1] != "\n"):
        raise SystemExit(f"row not found (terminator-anchored): {row_id}")
    # pull in comment lines directly above the row -- they describe it and must not orphan
    start = i
    while True:
        prev = text.rfind("\n", 0, start - 1) + 1
        if text[prev:start].lstrip().startswith("#") and prev < start:
            start = prev
        else:
            break
    j = text.find("\n- id: ", i)
    end = len(text) if j < 0 else j + 1
    return text[:start] + text[end:], text[i:end]


def main() -> None:
    ids = sys.argv[1:]
    if not ids:
        raise SystemExit(__doc__)
    board = open(BOARD).read()
    moved = []
    for row_id in ids:
        board, block = cut_row(board, row_id)
        for old in ("status: delegated", "status: todo"):
            block = block.replace(old, "status: done")
        moved.append(block.rstrip() + "\n")
    archive = open(ARCHIVE).read().rstrip() + "\n"
    archive += "\n" + "\n".join(moved)
    rows = yaml.safe_load(board)
    arch = yaml.safe_load(archive)
    live_ids = {r["id"] for r in rows}
    arch_ids = [r["id"] for r in arch]
    for row_id in ids:
        assert row_id not in live_ids, f"{row_id} still live"
        assert arch_ids.count(row_id) == 1, f"{row_id} not exactly once in archive"
    open(BOARD, "w").write(board)
    open(ARCHIVE, "w").write(archive)
    print(f"archived: {', '.join(ids)} ({len(rows)} live, {len(arch)} archived)")


if __name__ == "__main__":
    main()
