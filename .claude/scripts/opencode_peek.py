#!/usr/bin/env python3
"""Colorized live view of an opencode --format json event stream (tick_stream's cousin).

process_line() is the reusable core: opencode_worker.py calls it in-process
while reading a live `opencode run` subprocess's stdout. main() keeps the old
`tail -n +1 -f opencode-114.jsonl | python3 opencode_peek.py` CLI shape
working for anyone invoking this file directly.
"""
import json
import sys

C = sys.stdout.isatty()


def paint(code, s):
    return f"\033[{code}m{s}\033[0m" if C else s


def gist(d, n=110):
    s = " ".join(f"{k}={v}" for k, v in d.items() if isinstance(v, (str, int)))
    s = s.replace("\n", " ")
    return s[:n] + ("…" if len(s) > n else "")


def process_line(line: str) -> None:
    try:
        e = json.loads(line)
    except json.JSONDecodeError:
        return
    t, p = e.get("type"), e.get("part", {})
    if t == "text":
        txt = (p.get("text") or "").replace("\n", " ").strip()
        if txt:
            print(paint("32", f". {txt[:300]}"))
    elif t == "tool_use":
        st = p.get("state", {})
        status = st.get("status", "")
        mark = "x" if status == "error" else "->"
        color = "31" if status == "error" else "36"
        print(paint(color, f"{mark} {p.get('tool', '?')} {gist(st.get('input', {}))}"))
        if status == "error":
            print(paint("31", f"   {str(st.get('error', ''))[:200]}"))
    elif t == "step_finish":
        u = p.get("tokens") or {}
        if u:
            print(paint("90", f"   step done in={u.get('input', 0)} out={u.get('output', 0)} "
                               f"cache_r={(u.get('cache') or {}).get('read', 0)}"))
    sys.stdout.flush()


def main() -> None:
    for line in sys.stdin:
        process_line(line)


if __name__ == "__main__":
    main()
