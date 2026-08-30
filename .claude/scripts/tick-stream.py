#!/usr/bin/env python3
"""Colorize a claude -p stream-json feed for the drive-loop terminal.

Reads events on stdin, prints a readable live trace, and writes the final
result event as JSON to argv[1] (drive-tick.sh's context watch reads it).
Prefixes: [tick] session line, "->" tool call, "." narration, "x" tool error,
"ok" final verdict.
"""
import json
import sys

OUT = sys.argv[1]


def c(code, s):
    return f"\x1b[{code}m{s}\x1b[0m" if sys.stdout.isatty() else s


def one_line(s, n):
    return " ".join(str(s).split())[:n]


for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        e = json.loads(line)
    except json.JSONDecodeError:
        continue
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
        with open(OUT, "w") as f:
            json.dump(e, f)
        verdict = one_line(e.get("result") or "", 300)
        print(c("1", f"ok {verdict}" if verdict else "ok done"))
    sys.stdout.flush()
