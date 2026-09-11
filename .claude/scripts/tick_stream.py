#!/usr/bin/env python3
"""Colorize a claude -p stream-json feed for the drive-loop terminal.

process_line() is the reusable core: drive_tick.py calls it in-process while
reading a live claude -p subprocess's stdout, and tick_peek.py calls it while
tailing a session transcript file. main() below keeps the old
`some-producer | python3 tick_stream.py OUT` CLI shape working for anyone
invoking this file directly.

Prefixes: [tick] session line, "->" tool call, "." narration, "x" tool error,
"ok" final verdict.
"""
import json
import sys


def c(code, s):
    return f"\x1b[{code}m{s}\x1b[0m" if sys.stdout.isatty() else s


def one_line(s, n):
    return " ".join(str(s).split())[:n]


def process_line(line: str, out_path: str) -> bool:
    """Render one stream-json line. Returns True if this was the terminal
    "result" event -- the caller must stop reading right away rather than
    waiting for EOF, which a leaked background-task fd can withhold forever
    (held the tick lock 90+ min, 2026-08-31, blocking every subsequent tick)."""
    line = line.strip()
    if not line:
        return False
    try:
        e = json.loads(line)
    except json.JSONDecodeError:
        return False
    t = e.get("type")
    if t == "system" and e.get("subtype") == "init":
        sid = (e.get("session_id") or "?")[:8]
        print(c("90", f"[tick] session {sid} · {e.get('model', '?')}"))
    elif t == "assistant":
        for b in e.get("message", {}).get("content", []):
            if b.get("type") == "text" and b.get("text", "").strip():
                print(c("32", ". ") + one_line(b["text"], 300))
            elif b.get("type") == "tool_use":
                inp = b.get("input", {})
                gist = (inp.get("description") or inp.get("command")
                        or inp.get("file_path") or inp.get("prompt") or "")
                print(c("36", f"-> {b.get('name', '?')} ") + c("90", one_line(gist, 100)))
    elif t == "user":
        content = e.get("message", {}).get("content")
        for b in content if isinstance(content, list) else []:
            if isinstance(b, dict) and b.get("type") == "tool_result" and b.get("is_error"):
                txt = b.get("content")
                if isinstance(txt, list):
                    txt = " ".join(x.get("text", "") for x in txt if isinstance(x, dict))
                print(c("31", "x ") + one_line(txt, 200))
    elif t == "result":
        with open(out_path, "w") as f:
            json.dump(e, f)
        verdict = one_line(e.get("result") or "", 300)
        print(c("1", f"ok {verdict}" if verdict else "ok done"))
        sys.stdout.flush()
        return True
    sys.stdout.flush()
    return False


def main() -> None:
    out_path = sys.argv[1]
    for line in sys.stdin:
        if process_line(line, out_path):
            break


if __name__ == "__main__":
    main()
