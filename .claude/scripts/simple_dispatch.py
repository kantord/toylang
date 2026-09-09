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


@dataclass
class Result:
    row_id: str
    ok: bool
    fatal: bool
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
        return True, f"credit check inconclusive ({e}), proceeding cautiously"
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
    f = open(LOCK_DIR / f"{row_id}.lock", "w")
    try:
        fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        f.close()
        return None
    f.write(str(os.getpid()))
    f.flush()
    return f


def dispatch_one(row_id: str, brief_path: Path, model: str, retry_cap: int,
                  snapshot: str, max_tokens: int) -> Result:
    lock = acquire_lock(row_id)
    if lock is None:
        return Result(row_id, False, False,
                       "another dispatch of this row is already running (lock held)", None)
    try:
        return _dispatch_one_locked(row_id, brief_path, model, retry_cap, snapshot, max_tokens)
    finally:
        fcntl.flock(lock, fcntl.LOCK_UN)
        lock.close()


def _dispatch_one_locked(row_id: str, brief_path: Path, model: str, retry_cap: int,
                          snapshot: str, max_tokens: int) -> Result:
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
        sh(["git", "clone", "--no-hardlinks", "--quiet", str(REPO), str(clone_dir)], env=env)
        sh(["git", "-C", str(clone_dir), "fetch", "origin", "-q"], env=env)
        sh(["git", "-C", str(clone_dir), "checkout", "--quiet", "-b", f"issue-{row_id}", "origin/main"], env=env)
        base_commit = sh(["git", "-C", str(clone_dir), "rev-parse", "HEAD"], env=env).stdout.strip()
        logline(f"cloned at {base_commit}")

        args = [str(MSB_BIN), "run", "-m", "16G", "-c", "4", "--no-tty", "-d",
                 "--name", name, "--secret", "OPENROUTER_API_KEY@openrouter.ai",
                 "--from-snapshot", snapshot, "--", "sh", "-c", "sleep infinity"]
        r = sh(args, env=env, timeout=120)
        if r.returncode != 0:
            logline(f"boot failed: {r.stderr}")
            return Result(row_id, False, True, f"sandbox boot failed: {r.stderr[:500]}", None)

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

        run_cmd = (
            f"cd /repo && export PATH=$HOME/.cargo/bin:/usr/lib/llvm-22/bin:$PATH && "
            f"export CARGO_BUILD_JOBS=2 && "
            f"timeout 1800 python3 /root/agent_loop.py --task-file /root/task.txt "
            f"--model {model} --retry-cap {retry_cap} --max-tokens {max_tokens} "
            f"> /root/agent.log 2>&1; "
            f"echo RC=$? >> /root/agent.log"
        )
        logline("running agent_loop.py")
        exec_in(run_cmd, timeout=1900)
        tail = exec_in("tail -c 8000 /root/agent.log").stdout
        logline(tail[-3000:])

        ok = "VERIFIED_GREEN" in tail
        fatal = "FATAL:" in tail

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
            patch_path = workdir / "result.patch"
            patch_path.write_text(patch_out)

        return Result(row_id, ok, fatal, tail[-1500:], patch_path)
    finally:
        sh([str(MSB_BIN), "rm", "-f", name], env=env)
        log.close()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("rows", nargs="+")
    ap.add_argument("--brief-dir", required=True, type=Path,
                     help="directory containing <row_id>.txt brief files")
    ap.add_argument("--model", default=DEFAULT_MODEL)
    ap.add_argument("--retry-cap", type=int, default=2)
    ap.add_argument("--max-tokens", type=int, default=4096)
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
                        args.snapshot, args.max_tokens): row
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
                results.append(Result(row, False, True, f"dispatch crashed: {e}", None))

    print("\n=== SUMMARY ===")
    for r in results:
        status = "FATAL" if r.fatal else ("GREEN" if r.ok else "RED")
        print(f"{r.row_id}: {status}" + (f" -> {r.patch_path}" if r.patch_path else ""))
    return 0 if all(r.ok for r in results) else 1


if __name__ == "__main__":
    sys.exit(main())
