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
import hashlib
import http.client
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
MESSAGES_FILE = "/root/agent-messages.json"

# Accumulated across every OpenRouter call this process makes (all attempts,
# all turns) -- a plain module-level global rather than threading a value
# through call_openrouter/agent_turns/check_fatal's call chain, since this
# is a single-purpose, single-process script and the alternative is
# plumbing an accumulator through several function signatures just to
# support a running total.
total_cost_usd = 0.0

# Identifies which original task this session's persisted messages belong
# to (see write_status()'s messages persistence and --resume-from below) --
# set once in main(), read wherever write_status() is called, including
# from inside call_openrouter's nested check_fatal(), which has no other
# way to know it.
_task_hash = ""


def add_cost(resp: dict) -> None:
    global total_cost_usd
    usage = resp.get("usage") or {}
    total_cost_usd += usage.get("cost") or 0.0


def write_status(status: str, messages: list | None = None) -> None:
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
    # On any non-GREEN exit, persist the full conversation so the $ already
    # spent exploring isn't silently thrown away -- a human can inspect why
    # it failed, or resume it later with --resume-from while the
    # provider-side prompt cache might still be warm. Never for GREEN --
    # nothing to recover from a success. task_hash lets --resume-from
    # refuse to load the wrong row's file instead of silently acting on an
    # unrelated session's history.
    if messages is not None and status != "GREEN":
        with open(MESSAGES_FILE, "w") as f:
            json.dump({"task_hash": _task_hash, "messages": messages}, f)


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
                write_status("FATAL", messages)
                sys.exit(2)

    try:
        with urllib.request.urlopen(req, timeout=180) as resp:
            raw = resp.read()
    except http.client.IncompleteRead as e:
        # A connection dropped mid-body AFTER the 200 headers already sent
        # is not an HTTPError and not an OSError/URLError/TimeoutError (
        # confirmed: IncompleteRead subclasses neither), so it fell through
        # every existing except clause here uncaught, past agent_turns'
        # `except RuntimeError`, crashing the process before write_status()
        # ever ran -- the same failure class already fixed for HTTPError,
        # transient network errors, and empty/missing choices below.
        print(f"incomplete response reading from OpenRouter: {e}", file=sys.stderr)
        raise RuntimeError(f"incomplete response: {e}") from e
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
    try:
        parsed = json.loads(text)
    except json.JSONDecodeError as e:
        # A 200 response whose body isn't valid JSON at all (an HTML error
        # page from a proxy/CDN sitting in front of OpenRouter, a stream
        # truncated in a way that still decodes as bytes but not as JSON)
        # raised this uncaught -- json.loads() was outside every try/except
        # in this function, past agent_turns' `except RuntimeError`,
        # crashing the process before write_status() ever ran. Same
        # treatment as every other OpenRouter-response failure mode here.
        print(f"non-JSON response from OpenRouter: {text[:500]}", file=sys.stderr)
        raise RuntimeError(f"non-JSON response: {e}") from e
    if isinstance(parsed, dict) and parsed.get("error"):
        err = parsed["error"]
        msg = err.get("message", str(err)) if isinstance(err, dict) else str(err)
        check_fatal(msg.lower(), msg)
        raise RuntimeError(f"OpenRouter returned an error body on HTTP 200: {msg}")
    # A 200 with no error field can still carry an empty/missing `choices`
    # (seen from upstream providers on content-moderation blocks and other
    # provider-side hiccups) -- confirmed directly: `{}["choices"][0]` and
    # `{"choices": []}["choices"][0]` both raise uncaught KeyError/IndexError
    # in agent_turns, past its `except RuntimeError`, killing the process
    # with a bare traceback before write_status ever runs. Raise the same
    # RuntimeError the sibling error-body case does so this is just another
    # ordinary retryable turn failure instead of an uncaught crash.
    if not isinstance(parsed, dict) or not parsed.get("choices"):
        raise RuntimeError(f"OpenRouter response missing choices: {text[:500]}")
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


def repo_state_signature(repo: str = "/repo") -> str:
    """A cheap fingerprint of the ACTUAL current repo content -- HEAD plus
    the full working-tree diff (tracked-file changes) and status
    (untracked/staged files). Used to detect real turn-to-turn (and, via
    agent_turns' `moved()`, real attempt-to-attempt) progress instead of a
    single fixed baseline: comparing against a FIXED `base_head`/
    `git_dirty()` pair captured once and never refreshed turned out to be
    permanently defeated the moment the tree first went dirty -- confirmed
    directly by simulating the loop: `git_dirty()` stays true forever once
    ANY edit lands and is never fully reverted (nothing resets the working
    tree between attempts by design), which reset the no-progress counter
    to 0 every single turn (or, for the equivalent bug one level up in
    main(), every single ATTEMPT) from then on regardless of whether
    anything further actually happened. Comparing this signature instead of
    a frozen baseline still resets correctly on a real further change, but
    keeps counting when nothing new happens to an already-dirty tree.

    The diff is hashed WITHOUT truncating it first -- this signature never
    leaves the process (nothing here is sent to OpenRouter or appended to
    `messages`, confirmed: it's hashed locally and discarded), so there is
    no token/dollar cost to capping it, only a correctness risk: an earlier
    version capped the diff at 50K chars before hashing, so two genuinely
    different large diffs sharing an identical first 50KB (e.g. the same
    already-modified file edited again further down) would have hashed
    identically and been misclassified as "no progress." Hashing locally is
    cheap regardless of size (sha256 over a few MB takes milliseconds), so
    there's no real reason to cap it."""
    r_head = subprocess.run(["git", "-C", repo, "rev-parse", "HEAD"],
                             text=True, capture_output=True, timeout=30)
    r_status = subprocess.run(["git", "-C", repo, "status", "--porcelain"],
                               text=True, capture_output=True, timeout=30)
    r_diff = subprocess.run(["git", "-C", repo, "diff", "HEAD"],
                             text=True, capture_output=True, timeout=30)
    blob = r_head.stdout + r_status.stdout + r_diff.stdout
    return hashlib.sha256(blob.encode(errors="replace")).hexdigest()


def safe_repo_state_signature(repo: str = "/repo") -> str | None:
    """repo_state_signature(), but never raises. Its three subprocess.run()
    calls carry a timeout (30s each) but nothing up the call chain ever
    caught a resulting TimeoutExpired/SubprocessError -- confirmed by
    adversarial review with a direct repro: an uncaught exception here
    propagates straight past agent_turns and main()'s only exception guard
    (`except OutOfTime`), killing the whole process before write_status()
    ever runs. simple_dispatch.py's fallback classifier only recognizes
    RC=124/137/143 (a raw `timeout`(1)/SIGKILL) as TIMEOUT, so a plain
    uncaught-Python-exception exit falls through to an indistinguishable
    "RED" -- exactly the "operator can't tell why it failed" failure class
    this whole rewrite exists to eliminate, just relocated to a spot the
    existing verify()-timeout handling doesn't cover. A real trigger is
    plausible given this design's own admitted constraints: the model has
    unrestricted run_bash and could leave `.git/index.lock` contention,
    background a `git gc`, or write a huge generated/binary file that makes
    `git diff HEAD` slow. Returns None on failure; callers must treat None
    as "unknown this check," never as a real signature value."""
    try:
        return repo_state_signature(repo)
    except (subprocess.SubprocessError, OSError) as e:
        print(f"  warning: repo_state_signature() failed ({e}) -- "
              "treating progress as unknown for this check rather than "
              "crashing the process", file=sys.stderr)
        return None


def _sig_changed(a: str | None, b: str | None) -> bool:
    """True iff two signatures are KNOWN to differ. If either is None (a
    transient git failure -- see safe_repo_state_signature), returns True:
    failing toward 'something changed' rather than toward 'nothing
    changed' avoids two bad outcomes on a mere hiccup -- wrongly advancing
    the no-progress-turns counter toward an early exit that has nothing to
    do with the model's actual behavior, and wrongly reporting `not moved`
    (which would discard this attempt's real transcript on a false
    premise). Worst case on a transient failure is one skipped
    optimization, never a wrong classification."""
    if a is None or b is None:
        return True
    return a != b


def agent_turns(api_key: str, model: str, messages: list, max_turns: int,
                 max_tokens: int, deadline: float,
                 max_turns_without_progress: int) -> tuple[str | None, bool]:
    """Runs up to max_turns tool-call rounds. Returns (final_text, moved):
    final_text is the model's final text once it stops calling tools, or
    None if max_turns was exhausted (or max_turns_without_progress
    consecutive turns passed with no actual repo change) without the model
    finishing; moved is True iff the repo's actual content differs at all
    between the START of THIS call and now. Raises OutOfTime if the
    wall-clock deadline passes first.

    `moved` is computed fresh (a direct repo_state_signature() comparison
    against this call's own starting signature) at every return point,
    deliberately NOT derived from the last per-turn signature seen inside
    the loop below -- that value always lags one turn behind (each turn
    checks progress made by the PREVIOUS turn, before running the current
    one), so reusing it here would silently miss progress made by the
    final turn of the attempt. main() uses this return value instead of
    its own `git_head()`/`git_dirty()` check against a fixed baseline --
    confirmed as a real, separate instance of the exact bug repo_state_signature()
    already fixed for the no-progress counter: a baseline fixed once before
    the WHOLE retry loop goes permanently 'moved' after the first real edit
    lands in ANY attempt, silently defeating the next attempt's own
    no-progress messages-reset even when that attempt itself does nothing.

    The no-progress cutoff exists because real data showed the ordinary
    per-turn budget alone doesn't catch the expensive failure shape: the
    real dense-tensor-type-build STUCK run burned $0.174 across 60 turns
    (2 full 30-turn attempts) and made ZERO file changes in either one --
    almost as expensive as a real shipped patch (toylang-conf-yaml-build's
    successful run cost $0.160), for no deliverable at all. Nothing
    previously noticed "N turns have gone by with no write_file taking
    effect" mid-attempt; the check was only ever done AFTER the full
    attempt (all max_turns) was already exhausted."""
    initial_sig = safe_repo_state_signature()

    def moved() -> bool:
        return _sig_changed(safe_repo_state_signature(), initial_sig)

    no_progress_turns = 0
    sig_failures = 0
    last_sig = initial_sig
    for turn in range(max_turns):
        if time.monotonic() > deadline:
            raise OutOfTime(f"wall-clock budget exhausted at turn {turn + 1}/{max_turns}")
        sig = safe_repo_state_signature()
        # `_sig_changed` deliberately fails toward "changed" whenever a
        # signature is None -- correct for a single transient git hiccup,
        # but adversarial review found a real, reproduced consequence: if
        # git fails on EVERY turn (a corrupted .git, disk full so every git
        # call ENOSPCs, a model-induced index.lock that never clears),
        # `_sig_changed(None, None)` is True every turn, `no_progress_turns`
        # resets to 0 forever, and the no-progress cutoff can never fire --
        # silently reintroducing the exact "explore forever, burn the whole
        # budget, never detected" shape this cutoff exists to close, just
        # triggered by SUSTAINED git failure instead of literal
        # zero-progress. Reproduced directly: 20 turns of an always-raising
        # repo_state_signature() ran all 20 turns instead of stopping at
        # max_turns_without_progress=3. Track sustained failures separately
        # and cut the attempt short on the same threshold once git itself
        # is the thing not working -- a single or occasional hiccup still
        # gets the safe "assume changed" treatment below.
        if sig is None:
            sig_failures += 1
            if sig_failures >= max_turns_without_progress:
                print(f"  repo state has been unreadable for {sig_failures} consecutive "
                      "turns (git itself appears broken), ending this attempt early -- "
                      "cannot verify progress either way", file=sys.stderr)
                return None, True
        else:
            sig_failures = 0
        if _sig_changed(sig, last_sig):
            no_progress_turns = 0
            last_sig = sig
        else:
            no_progress_turns += 1
            if no_progress_turns >= max_turns_without_progress:
                print(f"  no repo changes for {no_progress_turns} consecutive turns, "
                      "ending this attempt early instead of spending the rest of "
                      "max_turns on further unproductive exploration", file=sys.stderr)
                return None, _sig_changed(sig, initial_sig)
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
            # Reuses `sig` (computed at the top of this same iteration)
            # instead of calling moved() again -- nothing between there and
            # here touches the filesystem (call_openrouter is a network
            # call), so it's already the freshest possible value.
            return None, _sig_changed(sig, initial_sig)
        usage = resp.get("usage", {})
        add_cost(resp)
        print(f"  turn {turn + 1}/{max_turns}: "
              f"prompt={usage.get('prompt_tokens')} completion={usage.get('completion_tokens')} "
              f"cost=${usage.get('cost', 0):.6f} (running total ${total_cost_usd:.6f})",
              file=sys.stderr)
        # Full usage dict, not just the three fields above -- a real
        # cost-review pass found $/1k-token swinging ~4x turn-to-turn in a
        # persisted log with no explanation from prompt/completion token
        # counts alone (e.g. dense-tensor-type-build turns 27 vs 29), and
        # couldn't tell whether that's provider-routing variance or
        # unlogged cache/reasoning-token billing because nothing captured
        # the rest of `usage` (cache read/write tokens, reasoning tokens,
        # which provider actually served the call). This is pulled into
        # RESULT_DIR via the existing full-agent.log copy, at zero added
        # cost (print, not an extra call) -- purely so the next round has
        # real data instead of guessing.
        print(f"    usage detail: {json.dumps(usage)}", file=sys.stderr)
        choice = resp["choices"][0]
        msg = choice["message"]
        messages.append(msg)
        tool_calls = msg.get("tool_calls") or []
        if not tool_calls:
            # Same reasoning as the RuntimeError-catch return above: no
            # tool_calls means nothing ran that could have touched the
            # filesystem this turn, so `sig` is still accurate.
            return msg.get("content") or "", _sig_changed(sig, initial_sig)
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
    return None, moved()


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
    #
    # BUT only when there's a second line to fall back on -- a genuinely
    # single-line tail (an early build/config error, a shell syntax error in
    # --verify-cmd, any crash before real test output starts, or even the
    # literal "(no changes, no verify run)" placeholder) has no truncation
    # fragment to drop; unconditionally dropping "line 0" left `all_lines[1:]`
    # empty, normalizing EVERY single-line tail to the same "" regardless of
    # actual content. Confirmed directly: two completely unrelated one-line
    # failures both normalized to "" and compared equal, misclassifying a
    # real difference as STUCK. The truncation-fragment assumption only
    # holds when there's more than one line to begin with.
    body_lines = all_lines[1:] if len(all_lines) > 1 else all_lines
    # Same class of bug as the first-line drop above, one filter further:
    # _NOISE_LINE_RE strips lines that look like a per-test PASS
    # announcement, but a single-line SUMMARY that happens to start with
    # the word "PASS" (e.g. "PASS: 5 FAIL: 2") matches it too. Confirmed
    # directly: two different single-line tails, "PASS: 5 FAIL: 2" and
    # "PASS: 3 FAIL: 9", both filtered down to nothing and compared equal --
    # the same false-STUCK-match failure mode the first-line fix closed,
    # one case narrower. Never let filtering erase ALL the signal: fall
    # back to the unfiltered body when the filtered result is empty but the
    # input wasn't -- on genuinely all-PASS-line input (rare and, if truly
    # identical, still compares correctly either way) this changes nothing
    # observable; on a single differing PASS-prefixed line it preserves the
    # real difference instead of discarding it.
    filtered = [l for l in body_lines if not _NOISE_LINE_RE.match(l)]
    lines = filtered if filtered else body_lines
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
    ap.add_argument("--max-turns-without-progress", type=int, default=12,
                     help="end an attempt early if this many consecutive turns pass with no "
                          "repo change at all (no new commit, nothing dirty) -- catches the "
                          "expensive failure shape a real run showed: 60 turns burning $0.174 "
                          "with zero file changes in either attempt, almost the cost of a real "
                          "shipped patch for no deliverable. Set higher than --max-turns to "
                          "disable")
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
    ap.add_argument("--resume-from", default=None,
                     help="path to a previously-persisted agent-messages.json (written by a "
                          "prior non-GREEN run). When set, --task-file is the NEW follow-up "
                          "instruction appended to that history, not a fresh task -- and "
                          "--original-task-file must also be given, so the loaded file's "
                          "task_hash can be checked against it. A mismatch means this points "
                          "at the wrong row's file; refuses to run rather than silently "
                          "acting on an unrelated session's history.")
    ap.add_argument("--original-task-file", default=None,
                     help="required with --resume-from: the ORIGINAL brief the resumed "
                          "session was for, used only to verify --resume-from points at the "
                          "right file")
    args = ap.parse_args()

    global _task_hash

    api_key = os.environ.get(args.api_key_env)
    if not api_key:
        print(f"FATAL: {args.api_key_env} not set in environment", file=sys.stderr)
        write_status("FATAL")
        return 2

    task_text = open(args.task_file).read()

    if args.resume_from:
        if not args.original_task_file:
            print("FATAL: --resume-from requires --original-task-file", file=sys.stderr)
            write_status("FATAL")
            return 2
        original_task_text = open(args.original_task_file).read()
        expected_hash = hashlib.sha256(original_task_text.encode()).hexdigest()
        try:
            with open(args.resume_from) as f:
                loaded = json.load(f)
        except (OSError, json.JSONDecodeError) as e:
            print(f"FATAL: --resume-from file unreadable/invalid JSON: {e}", file=sys.stderr)
            write_status("FATAL")
            return 2
        if not isinstance(loaded, dict) or "messages" not in loaded or "task_hash" not in loaded:
            print("FATAL: --resume-from file is not a valid persisted session "
                  "(missing task_hash/messages)", file=sys.stderr)
            write_status("FATAL")
            return 2
        if loaded["task_hash"] != expected_hash:
            print("FATAL: --resume-from task_hash does not match --original-task-file -- "
                  "this looks like a DIFFERENT row/task's persisted session. Refusing to "
                  "resume with unrelated history rather than guessing.", file=sys.stderr)
            write_status("FATAL")
            return 2
        messages = loaded["messages"]
        if not isinstance(messages, list) or not all(
            isinstance(m, dict) and "role" in m for m in messages
        ):
            print("FATAL: --resume-from messages are malformed", file=sys.stderr)
            write_status("FATAL")
            return 2
        print(f"== resuming {len(messages)} persisted messages -- any provider-side prompt "
              "cache benefit depends on how long ago the original run ended; if it's been a "
              "while, this may cost as much as a fresh start ==", file=sys.stderr)
        messages.append({
            "role": "user",
            "content": (
                "Your task has been rescoped based on what you already learned in this "
                f"session:\n\n{task_text}\n\n"
                "IMPORTANT: this is a fresh checkout. Any edits you believe you made above "
                "may or may not actually be present in this repo -- the caller may or may not "
                "have reapplied your prior patch. Run `git log` / `git diff` / `git status` "
                "yourself FIRST and act on what you actually find, not on memory of previous "
                "write_file results."
            ),
        })
        _task_hash = expected_hash
    else:
        _task_hash = hashlib.sha256(task_text.encode()).hexdigest()
        messages = [
            {"role": "system", "content": SYSTEM_PROMPT},
            {"role": "user", "content": f"TASK:\n{task_text}\n\nVerify with: `{args.verify_cmd}`"},
        ]

    deadline = time.monotonic() + args.wall_clock_budget
    attempt = 0
    seen_tails = []
    while True:
        attempt += 1
        print(f"== attempt {attempt}/{args.retry_cap + 1} ==", file=sys.stderr)
        # Marker for where THIS attempt's own additions begin -- used below
        # to discard only those on a no-progress outcome, never anything
        # from an earlier attempt. Discarding back to a fixed index (2, or
        # messages[2:]) was a real bug found by adversarial review: if
        # attempt 1 makes a genuine, proven-valuable edit (moved=True, RED,
        # kept) and attempt 2 builds on it but adds nothing further of its
        # own (moved=False relative to ATTEMPT 2's own start), resetting to
        # a fixed index-2 wiped out attempt 1's entire real transcript too.
        #
        # Stored as the LAST MESSAGE OBJECT itself (identity, via `is`),
        # not its numeric index -- a second real bug, found on the very
        # next adversarial round: `trim_messages()` runs every turn and can
        # delete whole turns starting at index 2 mid-attempt, shifting
        # every later index down. A captured absolute index doesn't move
        # with it; reproduced directly: a long attempt's own tool output
        # pushes the conversation past MAX_CONVERSATION_CHARS, trims fire
        # mid-attempt, and `del messages[stale_index:]` then cuts in the
        # middle of a turn -- leaving an assistant `tool_calls` message
        # with no matching `tool` reply, which OpenRouter rejects on the
        # NEXT call, corrupting every remaining attempt with the same
        # unrelated failure. An object reference survives being shifted;
        # `trim_messages()` only ever removes WHOLE turns, so any surviving
        # message is always still a valid turn boundary to cut after.
        attempt_start_marker = messages[-1]
        try:
            final_text, moved = agent_turns(api_key, args.model, messages, args.max_turns,
                                             args.max_tokens, deadline,
                                             args.max_turns_without_progress)
        except OutOfTime as e:
            print(f"OUT_OF_TIME: {e}", file=sys.stderr)
            write_status("TIMEOUT", messages)
            return 3
        if final_text is None:
            print("== ran out of turns without the model finishing ==", file=sys.stderr)

        # `moved` comes from agent_turns' own per-ATTEMPT signature
        # comparison now, not a `git_head()`/`git_dirty()` check against a
        # baseline fixed once before this whole while-loop -- that fixed
        # baseline was a real, separate instance of the exact bug
        # repo_state_signature() already fixed for the no-progress counter:
        # once ANY attempt made a real edit, `git_dirty()` stayed true
        # forever, so `moved` was permanently True for every LATER attempt
        # too, even one that itself changed nothing -- silently defeating
        # the messages-reset below for every attempt after the first real
        # edit. Confirmed by adversarial review as a direct re-occurrence of
        # the bug fixed one level down.
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
                write_status("TIMEOUT", messages)
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
                write_status("TIMEOUT", messages)
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
            write_status("STUCK", messages)
            return 1
        seen_tails.append(normalized)

        if attempt > args.retry_cap:
            print("VERIFY_FAILED")
            print(tail)
            write_status("RED", messages)
            return 1

        if moved:
            feedback = (
                "The verification command failed. Fix the SPECIFIC failures below --"
                " do not start over or redo work that already passed. Re-run "
                f"`{args.verify_cmd}` yourself before claiming DONE again.\n\n{tail}"
            )
        else:
            # An attempt that made ZERO repo changes has a fruitless
            # exploration history with proven zero value -- nothing was
            # kept, there is no diff, there is nothing for the next attempt
            # to build on. Carrying it forward only re-pays for it: real
            # numbers from dense-tensor-type-build's persisted log show its
            # attempt-2 no-progress window (turns 1-12, identical outcome
            # to attempt-1's) cost $0.049085 vs attempt-1's $0.014439 for
            # the SAME zero-progress result -- 3.4x more, purely from
            # carried context. Reset back to `attempt_start_marker` -- NOT a
            # fixed `messages[2:]` -- so the next attempt starts cheap
            # instead of re-billing a transcript that led nowhere, without
            # also discarding any EARLIER attempt's real, proven-valuable
            # progress (messages[0]/[1] are never touched either way, same
            # invariant trim_messages() keeps).
            #
            # Deliberately done HERE -- only once we know this run is
            # actually continuing to another attempt -- and not immediately
            # after computing `moved` above: this exact attempt's own
            # transcript still needs to reach write_status() (STUCK/RED)
            # untouched on every EXIT path above (including the final
            # attempt's own STUCK/RED), so a human or propose_narrower_task
            # can see what was actually tried. Resetting before those
            # write_status() calls would have persisted an already-gutted
            # messages list for the very attempt whose failure is being
            # reported -- confirmed as a real bug by adversarial review.
            #
            # Looked up by IDENTITY, not a stored index -- trim_messages()
            # may have shifted everything since the marker was captured. If
            # the marker itself is gone (only possible if THIS attempt's
            # own growth was large enough that trim_messages() discarded
            # even its own starting point -- an extreme case given
            # --max-turns-without-progress should end a truly unproductive
            # attempt long before that much output accumulates), there is
            # no longer a safe, turn-aligned boundary that discards ONLY
            # this attempt's content without risking an orphaned
            # `tool_calls` message -- confirmed as a real, reproduced bug
            # when this used a stale absolute index instead. Skip the reset
            # entirely rather than guess: carrying the (already
            # trim-bounded) history forward is safe, corrupting it is not.
            for _i, _m in enumerate(messages):
                if _m is attempt_start_marker:
                    del messages[_i + 1:]
                    break
            feedback = (
                "Your previous attempt ended without making any repo changes at all "
                "(either it never edited anything, or it hit the no-progress cutoff). "
                "Try a different, more direct approach this time -- start by editing, "
                "not just exploring.\n\nTASK:\n" + task_text
            )
        messages.append({"role": "user", "content": feedback})


if __name__ == "__main__":
    sys.exit(main())
