#!/usr/bin/env python3
"""Minimal autonomous coding agent. Runs INSIDE a sandbox, talks directly to
OpenRouter's chat-completions API (stdlib only, no `opencode` CLI). Replaces
the opencode-based build loop that sandbox_dispatch.py drove, whose bugs all
came from opencode's own session/CLI machinery: stdin hangs without
`< /dev/null`, `--continue` reading stale on-disk sessions, OPENCODE_MODEL not
persisting across invocations, `--agent plan/build` mode selection, canned
per-phase prompt templates, and a discarded verify() result on the "split"
path. None of that machinery exists here: one process, one in-memory message
list, one verify loop. Verification always drives the next step because
there is only one place it happens.

Usage:
  agent_loop.py --task-file brief.txt [--model deepseek/deepseek-v4-flash-0731]
      [--max-turns 30] [--retry-cap 2] [--verify-cmd "just check"]

Exit codes: 0 = verified green. 1 = ran out of retries, still red (patch may
still be worth extracting -- caller decides). 2 = FATAL (bad key, no credit,
etc) -- do not retry this, something external needs fixing first. 3 = ran out
of wall-clock budget mid-attempt -- distinct from a real RED; the task may
need a bigger budget, not a different fix.
"""
from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request

API_URL = "https://openrouter.ai/api/v1/chat/completions"
MAX_TOOL_OUTPUT = 8000  # chars; keeps context from ballooning turn over turn

SYSTEM_PROMPT = """You are an autonomous coding agent working in /repo (a git checkout).
You have three tools: read_file, write_file, run_bash. Use run_bash to explore
(ls, grep, cat) and to run the verification command yourself as often as you like.
Use write_file to make edits -- it overwrites the whole file, so read it first if
you're editing rather than creating.

When you believe the task is fully done AND you have personally run the
verification command and seen it pass, reply with no tool calls and a message
starting with "DONE:". Do not claim DONE without having actually run and seen
the verification command succeed in this session -- the harness re-runs it
independently and will not take your word for it.
"""

TOOLS = [
    {
        "type": "function",
        "function": {
            "name": "read_file",
            "description": "Read a file's full contents.",
            "parameters": {
                "type": "object",
                "properties": {"path": {"type": "string"}},
                "required": ["path"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "write_file",
            "description": "Overwrite a file with the given content (creates it if missing).",
            "parameters": {
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"},
                },
                "required": ["path", "content"],
            },
        },
    },
    {
        "type": "function",
        "function": {
            "name": "run_bash",
            "description": "Run a shell command in /repo and get back stdout+stderr.",
            "parameters": {
                "type": "object",
                "properties": {"command": {"type": "string"}},
                "required": ["command"],
            },
        },
    },
]

# Substrings meaning the API call itself failed for a reason no amount of
# retrying will fix -- same class the old harness's FATAL_API_PATTERNS caught,
# checked directly against the HTTP response instead of grepping an
# opencode-authored log file.
FATAL_PATTERNS = (
    "insufficient_quota", "insufficient credit", "requires more credits",
    "invalid_api_key", "api key expired", "no auth credentials found",
)


MAX_CONVERSATION_CHARS = 200_000  # keeps context from growing without bound
                                   # across many turns/attempts on a long task


STATUS_FILE = "/root/agent-status.txt"
COST_FILE = "/root/agent-cost.txt"

# Accumulated across every OpenRouter call this process makes (all attempts,
# all turns) -- a plain module-level global rather than threading a value
# through call_openrouter/agent_turns/check_fatal's call chain, since this
# is a single-purpose, single-process script and the alternative is
# plumbing an accumulator through several function signatures just to
# support a running total.
total_cost_usd = 0.0


def add_cost(resp: dict) -> None:
    global total_cost_usd
    usage = resp.get("usage") or {}
    total_cost_usd += usage.get("cost") or 0.0


def write_status(status: str) -> None:
    """The single source of truth simple_dispatch.py reads for outcome
    classification -- NOT a substring grep over the shared stdout/stderr
    log. That log can and does contain model-generated tool output (a
    `grep`/`cat` over this very repo could echo back "VERIFIED_GREEN",
    "FATAL:", or "RC=124" verbatim, and MAX_TOOL_OUTPUT=8000 is exactly the
    size of simple_dispatch.py's classification window), so any substring
    check against it is one unlucky tool call away from a false positive or
    negative. This file's content is written ONLY by this function, never
    by echoing anything the model or a tool produced, so an exact-match read
    of it can't collide with arbitrary text."""
    with open(STATUS_FILE, "w") as f:
        f.write(status)
    # Written alongside status, every time, regardless of outcome -- the
    # CSV dispatch log needs a real cost even for FATAL/TIMEOUT/STUCK runs,
    # not just GREEN ones.
    with open(COST_FILE, "w") as f:
        f.write(f"{total_cost_usd:.6f}")


def truncate(s: str, n: int = MAX_TOOL_OUTPUT) -> str:
    if len(s) <= n:
        return s
    return s[:n] + f"\n... [truncated, {len(s) - n} more chars]"


def trim_messages(messages: list) -> None:
    """Drops the OLDEST complete turns (never messages[0] system prompt or
    messages[1] the original task) once the conversation gets too big,
    instead of letting it grow unboundedly across many turns and retry
    attempts until a request fails on the model's own context limit --
    which would silently burn the rest of the retry budget on calls that
    can never succeed. A "turn" is one assistant message plus every tool
    reply that answers its tool_calls, or a single plain message; units are
    dropped whole so a tool reply is never left orphaned from its call."""
    while len(json.dumps(messages)) > MAX_CONVERSATION_CHARS and len(messages) > 3:
        first = messages[2]
        end = 3
        n_calls = len(first.get("tool_calls") or [])
        while n_calls > 0 and end < len(messages) and messages[end].get("role") == "tool":
            end += 1
            n_calls -= 1
        del messages[2:end]


def run_tool(name: str, args: dict) -> str:
    try:
        if name == "read_file":
            with open(args["path"], "r", errors="replace") as f:
                return truncate(f.read())
        if name == "write_file":
            path = args["path"]
            d = os.path.dirname(path)
            if d:
                os.makedirs(d, exist_ok=True)
            with open(path, "w") as f:
                f.write(args["content"])
            return f"wrote {len(args['content'])} bytes to {path}"
        if name == "run_bash":
            # start_new_session=True puts the command (and anything it
            # backgrounds with & or nohup) in its own process group, so a
            # `./server & disown`-style call can be fully killed once the
            # foreground shell exits, instead of leaking an orphaned process
            # that holds a port/file across the rest of this attempt and
            # into retries.
            #
            # Deliberately NOT proc.communicate(timeout=...): communicate()
            # blocks until the pipe's write end is closed by EVERY process
            # that inherited it, including a backgrounded child that never
            # touches it again -- confirmed directly, a `sleep 30 &` job
            # made communicate() hang for the full timeout even though the
            # foreground shell returned in milliseconds. Poll for the
            # foreground shell's own exit instead, kill its whole process
            # group the moment it's done (or the timeout elapses), and only
            # then read the pipe -- by then every writer is dead and the
            # read reaches EOF immediately instead of blocking on a child
            # the caller never waited for.
            proc = subprocess.Popen(
                args["command"], shell=True, cwd="/repo", text=True,
                stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                start_new_session=True,
            )
            deadline = time.monotonic() + 300
            while proc.poll() is None and time.monotonic() < deadline:
                time.sleep(0.05)
            timed_out = proc.poll() is None
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            try:
                out_text = proc.stdout.read() if proc.stdout else ""
            except Exception:
                out_text = ""
            rc = "timeout" if timed_out else proc.returncode
            out = f"$ {args['command']}\n(exit {rc})\n{out_text}"
            return truncate(out)
        return f"unknown tool: {name}"
    except Exception as e:
        return f"tool error: {e}"


def call_openrouter(api_key: str, model: str, messages: list, max_tokens: int) -> dict:
    body = json.dumps({
        "model": model,
        "messages": messages,
        "tools": TOOLS,
        "tool_choice": "auto",
        # Without this, OpenRouter defaults max_tokens to the model's full
        # context window and refuses the WHOLE call if the account can't
        # cover that theoretical ceiling -- confirmed live: a request with no
        # max_tokens was rejected as unaffordable at "up to 131072 tokens"
        # even though a normal-sized completion would have fit easily.
        "max_tokens": max_tokens,
        # Confirmed live: without this, `usage.cost` is absent from the
        # response entirely. With it, every response reports the real
        # dollar cost of that specific call -- this is what makes the CSV
        # dispatch log's cost column real data instead of an estimate.
        "usage": {"include": True},
    }).encode()
    req = urllib.request.Request(
        API_URL, data=body,
        headers={
            "Authorization": f"Bearer {api_key}",
            "Content-Type": "application/json",
        },
        method="POST",
    )
    def check_fatal(payload_lower: str, payload: str):
        for pat in FATAL_PATTERNS:
            if pat in payload_lower:
                print(f"FATAL: {pat} -- {payload[:500]}", file=sys.stderr)
                write_status("FATAL")
                sys.exit(2)

    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            raw = resp.read()
    except urllib.error.HTTPError as e:
        payload = e.read().decode(errors="replace")
        check_fatal(payload.lower(), payload)
        print(f"HTTP {e.code} calling OpenRouter: {payload[:1000]}", file=sys.stderr)
        # A non-fatal HTTP error (429 rate-limit, 500/502/503 gateway
        # hiccup, a transient 400) is NOT the same as FATAL (already handled
        # above via check_fatal/sys.exit) -- it's a normal retryable turn
        # failure, same as the URLError/OSError branch below. Re-raising the
        # bare HTTPError here instead of wrapping it meant agent_turns'
        # `except RuntimeError` never caught it: the process crashed with an
        # uncaught traceback mid-attempt, before verify() ever ran, wasting
        # the whole sandbox run on what a single 429 (very plausible with
        # --parallel hammering one shared key) should have just retried.
        raise RuntimeError(f"HTTP {e.code} error: {payload[:500]}") from e
    except (urllib.error.URLError, OSError, TimeoutError) as e:
        # A transient network failure (DNS blip, connection reset, timeout)
        # is not fatal and not success -- surface it as a normal exception so
        # the caller's retry-cap loop can treat it the same as any other
        # attempt that didn't reach GREEN, instead of crashing the process
        # with no VERIFIED_GREEN/VERIFY_FAILED/FATAL marker at all (which the
        # host side would otherwise silently read as a plain, unexplained RED).
        print(f"transient network error calling OpenRouter: {e}", file=sys.stderr)
        raise RuntimeError(f"transient network error: {e}") from e

    # OpenRouter can return HTTP 200 with an embedded {"error": ...} body
    # (e.g. an upstream provider failure) -- this must be checked BEFORE
    # indexing choices[0], or an out-of-credit condition surfacing this way
    # crashes with an unhandled KeyError instead of being classified at all,
    # silently reintroducing the exact "dispatch into a dead account"
    # failure mode this script exists to catch.
    text = raw.decode(errors="replace")
    parsed = json.loads(text)
    if isinstance(parsed, dict) and parsed.get("error"):
        err = parsed["error"]
        msg = err.get("message", str(err)) if isinstance(err, dict) else str(err)
        check_fatal(msg.lower(), msg)
        raise RuntimeError(f"OpenRouter returned an error body on HTTP 200: {msg}")
    return parsed


class OutOfTime(Exception):
    """Raised when the wall-clock budget runs out mid-attempt. Caught in
    main() and reported as its own distinct outcome -- deliberately NOT the
    same as a genuine RED, so an operator (or a future automated escalation)
    can tell "the task is probably too big for this budget" apart from "the
    model tried and failed." Letting the outer OS-level `timeout` SIGKILL the
    process instead would produce neither VERIFIED_GREEN nor VERIFY_FAILED in
    the log at all -- indistinguishable from a plain unexplained RED, exactly
    the "operator can't tell why it failed" shape this rewrite exists to
    avoid."""


def agent_turns(api_key: str, model: str, messages: list, max_turns: int,
                 max_tokens: int, deadline: float) -> str | None:
    """Runs up to max_turns tool-call rounds. Returns the model's final text
    once it stops calling tools, or None if max_turns was exhausted without
    the model finishing. Raises OutOfTime if the wall-clock deadline passes
    first."""
    for turn in range(max_turns):
        if time.monotonic() > deadline:
            raise OutOfTime(f"wall-clock budget exhausted at turn {turn + 1}/{max_turns}")
        try:
            resp = call_openrouter(api_key, model, messages, max_tokens)
        except RuntimeError as e:
            # Transient network failure or a 200-with-error-body from
            # OpenRouter -- not fatal (that already exited via sys.exit(2)
            # inside call_openrouter), just this turn failing. Stop this
            # attempt here and let main()'s retry-cap loop treat it like any
            # other incomplete attempt, rather than crashing the process.
            print(f"  turn {turn + 1}/{max_turns}: call failed ({e}), ending this attempt",
                  file=sys.stderr)
            return None
        usage = resp.get("usage", {})
        add_cost(resp)
        print(f"  turn {turn + 1}/{max_turns}: "
              f"prompt={usage.get('prompt_tokens')} completion={usage.get('completion_tokens')} "
              f"cost=${usage.get('cost', 0):.6f} (running total ${total_cost_usd:.6f})",
              file=sys.stderr)
        choice = resp["choices"][0]
        msg = choice["message"]
        messages.append(msg)
        tool_calls = msg.get("tool_calls") or []
        if not tool_calls:
            return msg.get("content") or ""
        for tc in tool_calls:
            fn = tc["function"]["name"]
            try:
                fn_args = json.loads(tc["function"]["arguments"] or "{}")
            except json.JSONDecodeError:
                fn_args = {}
            result = run_tool(fn, fn_args)
            messages.append({
                "role": "tool",
                "tool_call_id": tc["id"],
                "content": result,
            })
        trim_messages(messages)
    return None


# Lines/tokens `cargo nextest` varies run-to-run even against a
# byte-identical failing tree: PASS lines (which tests land in the tail at
# all depends on parallel-completion order, not just which tests exist),
# per-test timings, thread ids in panic lines, and the running "N/total"
# position counter. Confirmed directly, not theoretically: two consecutive
# `just check` runs against the SAME deliberately-broken test produced
# different tails (different PASS lines preceding the FAIL, a different
# passed-count in the Summary line) purely from parallel scheduling
# nondeterminism -- which would have made the original exact-string STUCK
# check silently never fire on exactly the "identical error 3 times" pattern
# it exists to catch. Stripping this noise before comparing keeps the actual
# diagnostic signal (FAIL lines, error/panic messages, summary counts of
# passed/failed) while ignoring what's provably nondeterministic per run.
_NOISE_LINE_RE = re.compile(r"^\s*PASS\b")
_TIMING_RE = re.compile(r"\[\s*[\d.]+s\]")
_POSITION_RE = re.compile(r"\(\s*\d+/\d+\)")
_THREAD_ID_RE = re.compile(r"\(\d{3,}\)")


def normalize_for_stuck_check(tail: str) -> str:
    all_lines = tail.splitlines()
    # The first line is very likely a mid-line fragment left by verify()'s
    # fixed-size [-6000:] slice, not a complete PASS/FAIL line -- it never
    # matches _NOISE_LINE_RE (which anchors on line start) and differs
    # between runs purely because of WHERE in an arbitrary PASS line the
    # slice happened to cut, not because of any real signal. Confirmed
    # directly: this was the one remaining diff after filtering whole PASS
    # lines out. Drop it; the content that matters is never at the very top
    # of a tail this size.
    lines = (l for l in all_lines[1:] if not _NOISE_LINE_RE.match(l))
    text = "\n".join(lines)
    text = _TIMING_RE.sub("[Ts]", text)
    text = _POSITION_RE.sub("(N/N)", text)
    text = _THREAD_ID_RE.sub("(PID)", text)
    return text


MAX_VERIFY_SECONDS = 1800


def verify(cmd: str, timeout: int = MAX_VERIFY_SECONDS) -> tuple[bool, str]:
    r = subprocess.run(cmd, shell=True, cwd="/repo", text=True,
                        capture_output=True, timeout=timeout)
    out = (r.stdout + r.stderr)[-6000:]
    return r.returncode == 0, out


def git_head(repo="/repo") -> str:
    return subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"],
                           text=True, capture_output=True, timeout=30).stdout.strip()


def git_dirty(repo="/repo") -> bool:
    r = subprocess.run(["git", "-C", repo, "status", "--porcelain"],
                        text=True, capture_output=True, timeout=30)
    return bool(r.stdout.strip())


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--task-file", required=True)
    ap.add_argument("--model", default="deepseek/deepseek-v4-flash-0731")
    ap.add_argument("--max-turns", type=int, default=30)
    ap.add_argument("--max-tokens", type=int, default=4096,
                     help="per-turn completion cap; keeps cost predictable and lets requests "
                          "succeed on a small remaining balance instead of being rejected for "
                          "the model's full context window")
    ap.add_argument("--retry-cap", type=int, default=2)
    ap.add_argument("--verify-cmd", default="just check")
    ap.add_argument("--api-key-env", default="OPENROUTER_API_KEY")
    ap.add_argument("--wall-clock-budget", type=int, default=3400,
                     help="seconds for turns across ALL attempts combined, checked before "
                          "each turn so this process can stop cleanly and report OUT_OF_TIME "
                          "instead of being SIGKILLed by an outer OS-level timeout with no "
                          "VERIFIED_GREEN/VERIFY_FAILED marker ever printed. The caller "
                          "(simple_dispatch.py) sizes its own outer timeout to leave room "
                          "for one more verify() call past this deadline -- keep this in sync "
                          "if you change verify()'s own timeout")
    args = ap.parse_args()

    api_key = os.environ.get(args.api_key_env)
    if not api_key:
        print(f"FATAL: {args.api_key_env} not set in environment", file=sys.stderr)
        write_status("FATAL")
        return 2

    task_text = open(args.task_file).read()
    messages = [
        {"role": "system", "content": SYSTEM_PROMPT},
        {"role": "user", "content": f"TASK:\n{task_text}\n\nVerify with: `{args.verify_cmd}`"},
    ]

    deadline = time.monotonic() + args.wall_clock_budget
    base_head = git_head()
    attempt = 0
    seen_tails = []
    while True:
        attempt += 1
        print(f"== attempt {attempt}/{args.retry_cap + 1} ==", file=sys.stderr)
        try:
            final_text = agent_turns(api_key, args.model, messages, args.max_turns,
                                      args.max_tokens, deadline)
        except OutOfTime as e:
            print(f"OUT_OF_TIME: {e}", file=sys.stderr)
            write_status("TIMEOUT")
            return 3
        if final_text is None:
            print("== ran out of turns without the model finishing ==", file=sys.stderr)

        moved = git_head() != base_head or git_dirty()
        if not moved:
            ok = False
            tail = "(no changes, no verify run)"
        else:
            # verify()'s own ceiling used to always be the full
            # MAX_VERIFY_SECONDS regardless of how much wall-clock budget
            # was actually left -- simple_dispatch.py's outer timeout only
            # ever reserved room for ONE such call, but retry_cap+1 attempts
            # each call verify() once, so 3 slow-but-not-hung verify passes
            # (each near its own cap) could blow past the total budget
            # without ever tripping the OutOfTime check in agent_turns
            # (which only runs before turns, not around verify()) --
            # confirmed by re-deriving the arithmetic, not by hitting it
            # live. Cap this call to whatever's actually left, and skip it
            # entirely (clean TIMEOUT, not a mid-verify SIGKILL) if there
            # isn't reasonably enough time to even try.
            remaining = deadline - time.monotonic()
            if remaining < 60:
                print("OUT_OF_TIME: not enough wall-clock budget left to run "
                      "another verify pass", file=sys.stderr)
                write_status("TIMEOUT")
                return 3
            try:
                ok, tail = verify(args.verify_cmd, timeout=min(MAX_VERIFY_SECONDS, int(remaining)))
            except subprocess.TimeoutExpired:
                # Capping verify()'s timeout to the remaining budget only
                # protects the wall clock if this is actually caught --
                # subprocess.run raises TimeoutExpired when the command
                # really does run that long (a genuine slow `just check`,
                # not a hang), and an uncaught exception here would crash
                # the process with no write_status call at all, exactly the
                # "TIMEOUT never actually gets reported" gap this whole
                # mechanism exists to close.
                print("OUT_OF_TIME: verify() itself exceeded the remaining "
                      "wall-clock budget", file=sys.stderr)
                write_status("TIMEOUT")
                return 3

        print(f"== verify: {'GREEN' if ok else 'RED'} ==", file=sys.stderr)
        print(tail[-2000:], file=sys.stderr)

        if ok:
            print("VERIFIED_GREEN")
            write_status("GREEN")
            return 0

        # The exact failure this rewrite was built to explain, not just cap:
        # the old harness burned its full retry budget hitting a
        # byte-identical compiler error 3 times in a row with zero progress.
        # Keeping the full message history across attempts (only trimmed by
        # size) doesn't prevent that on its own -- a cheap, precise, direct
        # check does: if the verify output hasn't changed AT ALL since the
        # last attempt, another retry with the same context is not going to
        # produce a different result. Stop immediately instead of spending
        # the rest of retry_cap re-deriving the same dead end, and report it
        # as its own distinct outcome so an operator can tell "genuinely
        # stuck, needs a different approach" apart from "still iterating."
        #
        # Checked against every PRIOR tail seen this run, not just the last
        # one -- comparing only to prev_tail misses an oscillating failure
        # (attempt 1 fails with A, attempt 2 with B, attempt 3 with A again),
        # which is fully reachable within the default retry_cap=2 (3
        # attempts) and would otherwise burn the whole retry budget without
        # ever being recognized as the same dead end recurring.
        #
        # Compared after normalize_for_stuck_check(), NOT the raw tail --
        # confirmed directly against this repo's real `just check`: two runs
        # of a byte-identical failing tree produced different raw tails
        # (parallel test scheduling changes which PASS lines land in the
        # last 6000 chars, plus per-test timings and the running position
        # counter), which would have made exact-tail-equality never fire on
        # the exact "identical error every attempt" pattern this check
        # exists to catch.
        normalized = normalize_for_stuck_check(tail)
        if normalized in seen_tails:
            print("STUCK: verify output matches a previous attempt, "
                  "not retrying further", file=sys.stderr)
            write_status("STUCK")
            return 1
        seen_tails.append(normalized)

        if attempt > args.retry_cap:
            print("VERIFY_FAILED")
            print(tail)
            write_status("RED")
            return 1

        feedback = (
            "The verification command failed. Fix the SPECIFIC failures below --"
            " do not start over or redo work that already passed. Re-run "
            f"`{args.verify_cmd}` yourself before claiming DONE again.\n\n{tail}"
        )
        messages.append({"role": "user", "content": feedback})


if __name__ == "__main__":
    sys.exit(main())
