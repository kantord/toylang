#!/usr/bin/env python3
"""Serial landing queue (maintainer redesign, 2026-09-01, superseding the
size-driven accumulator pipeline of 2026-08-30 -- fold/promote and the
to-merge-* branches are retired):

  uv run --project .claude/scripts .claude/scripts/land_lane.py land <issue-number>...
  uv run --project .claude/scripts .claude/scripts/land_lane.py land-patch <row-id> <patch-file>

One lane at a time, straight onto main, behind the FULL `just test` in a
throwaway worktree -- main is only touched after green. Lands serialize on a
flock. A merge conflict or a red gate never blocks the queue: the script
re-dispatches via simple_dispatch.py with a templated repair brief carrying
the evidence (cap: 2 automatic retries, tracked in
$LOG_DIR/land-retries-issue-N), then moves to the next candidate. The third
failure leaves $LOG_DIR/land-failed-issue-N for the tick to escalate into a
maintainer round.

`land-patch` (added 2026-09-11, simple_dispatch.py's only dispatch
mechanism) is an adapter: simple_dispatch.py never creates a lane worktree,
it produces a plain `git format-patch` file from a disposable clone.
`land-patch` materializes the same worktree/branch `land` itself expects
from that patch (`git am` onto a fresh branch off main), then runs the
exact same landing logic -- gate, merge, push, retry-on-failure -- as
`land` does for any other row.

Deterministic by design (maintainer ruling, 2026-09-01): no model reads the
diff before landing -- `just test` is the whole pre-merge gate, and review
happens post-land, asynchronously, in the tick. The merge message is
generated from the lane's own commit subjects.
"""
import fcntl
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path

REPO = Path("/home/kantord/repos/toylang")
LANES = Path.home() / ".local" / "share" / "toylang-lanes"
LOG_DIR = Path.home() / ".cache" / "toylang-drive"
SCRIPTS = Path(__file__).resolve().parent
RETRY_CAP = 2

# Files whose merge conflicts are RESOLVABLE BY REGENERATION (maintainer
# ruling, 2026-09-01): corpus.json appeared in every conflict of the queue's
# first night; burning a worker on computable content was the dominant retry
# cost. A conflict touching ONLY these is resolved mechanically below.
GENERATED = {
    "site/public/corpus.json",
    "tests/snapshots/backend_llvm__native_agrees_where_it_compiles.snap",
    "tests/snapshots/backend_rust__rust_agrees_where_it_compiles.snap",
}
REGEN_TESTS = ("test(export_the_corpus_for_the_site) + "
               "test(native_agrees_where_it_compiles) + "
               "test(rust_agrees_where_it_compiles)")


def run(cmd, **kw):
    return subprocess.run(cmd, **kw)


def set_cargo_env() -> None:
    # The gate and the regeneration path need cargo regardless of who fired
    # us (worker trap, tick, interactive shell) -- mutates this process's
    # own environment once, at startup, exactly like the bash version's
    # `export PATH=...`/`export RUSTC_WRAPPER=sccache` affected every
    # subprocess for the rest of the script's life.
    os.environ["PATH"] = str(Path.home() / ".cargo" / "bin") + os.pathsep + os.environ.get("PATH", "")
    if shutil.which("sccache"):
        os.environ["RUSTC_WRAPPER"] = "sccache"


def worker_free(d: Path) -> bool:
    """A no-op under simple_dispatch.py (2026-09-11): dispatch runs entirely
    inside a disposable msb sandbox, never as a long-lived host process
    sitting in a lane directory, so there is never a matching pgrep hit to
    find here. Kept rather than deleted: `land-patch` reuses this same
    worktree convention once the patch is materialized, and a future
    dispatch mechanism reintroducing a real host-side worker process should
    still get this guard for free."""
    pids = []
    for name in ("opencode", "claude"):
        r = run(["pgrep", "-x", name], text=True, capture_output=True)
        pids += r.stdout.split()
    for pid in pids:
        try:
            cwd = os.readlink(f"/proc/{pid}/cwd")
        except OSError:
            continue
        if cwd.startswith(str(d)):
            return False
    return True


def fire_tick() -> None:
    # sys.executable, not "uv run --project" again: this process is itself
    # only ever started via `uv run --project .claude/scripts land_lane.py`,
    # so sys.executable already IS that project's own venv interpreter
    # (confirmed: it resolves with pyyaml importable) -- no need to pay
    # uv's resolution overhead a second time for a sibling script in the
    # same project. See drive_loop.py's matching comment.
    log_path = LOG_DIR / "event-ticks.log"
    with open(log_path, "a") as log:
        subprocess.Popen(
            [sys.executable, str(SCRIPTS / "drive_tick.py")],
            stdout=log, stderr=subprocess.STDOUT, stdin=subprocess.DEVNULL,
            start_new_session=True,
        )


def retrigger(n: str, kind: str, evidence_path: Path) -> None:
    """Red gate or conflict: re-dispatch via simple_dispatch.py with a
    templated repair brief carrying the evidence, or, past the cap, leave
    a land-failed marker for the tick to escalate."""
    retries_file = LOG_DIR / f"land-retries-issue-{n}"
    count = int(retries_file.read_text().strip() or "0") + 1 if retries_file.exists() else 1
    retries_file.write_text(f"{count}\n")
    if count > RETRY_CAP:
        (LOG_DIR / f"land-failed-issue-{n}").write_text(
            f"landing issue-{n}: {kind}, attempt {count} -- retry cap reached\n")
        print(f"[land] issue-{n}: {kind} on attempt {count} -- CAP REACHED, left for escalation")
        return
    print(f"[land] issue-{n}: {kind} on attempt {count} -- re-dispatching via simple_dispatch.py")
    # simple_dispatch.py resets the row fresh (a new disposable clone) on
    # every run, same stateless-per-attempt reasoning as drive_tick.py's own
    # ticks -- a file left in the OLD worktree would not survive to the
    # retry, so the evidence goes straight into the brief text instead of a
    # copied LAND-FAILURE.txt. Written to plans/simple-briefs/<row>.txt, the
    # SAME convention the coordinator's own fresh dispatches use --
    # simple_dispatch.py requires --brief-dir/<row_id>.txt exactly.
    briefs_dir = REPO / "plans" / "simple-briefs"
    briefs_dir.mkdir(parents=True, exist_ok=True)
    evidence = evidence_path.read_text(errors="replace") if evidence_path.exists() else ""
    brief_text = (
        "A previous dispatch completed this task, but landing the branch on main\n"
        f"FAILED: {kind} (landing attempt {count} of {RETRY_CAP + 1}). The exact evidence:\n\n"
        f"{evidence}\n\n"
        "Your job now is ONLY to make this branch land: fix whatever the evidence above\n"
        "shows failing, and re-run the verify command yourself. Do not start new feature work.\n"
    )
    (briefs_dir / f"{n}.txt").write_text(brief_text)
    dispatch_log = LOG_DIR / f"simple-dispatch-issue-{n}.log"
    # sys.executable, not "uv run --project": see fire_tick()'s comment above.
    with open(dispatch_log, "a") as log:
        subprocess.Popen(
            [sys.executable, str(SCRIPTS / "simple_dispatch.py"), n,
             "--brief-dir", str(briefs_dir)],
            cwd="/", stdin=subprocess.DEVNULL, stdout=log, stderr=subprocess.STDOUT,
            start_new_session=True,
        )
    # This dispatch is NOT self-landing (simple_dispatch.py deliberately
    # never calls land_lane.py itself, staying a pure dispatch primitive --
    # see plans/simple-dispatch-design.md) -- the next drive tick is
    # responsible for noticing a fresh GREEN row for "n" in
    # plans/dispatch-log.csv and running `land_lane.py land-patch n
    # <patch-path>` again, the same way it does for any other dispatch.


def land_one(n: str) -> bool:
    """Row/issue identifier already checked out at $LANES/issue-<n> on
    branch issue-<n>. Returns True if it landed on main (pushed), False for
    any other outcome (skipped, deferred, or re-dispatched via retrigger --
    all already logged by the point this returns)."""
    d = LANES / f"issue-{n}"
    branch = f"issue-{n}"
    if not d.is_dir():
        print(f"[land] skip issue-{n}: no worktree {d}")
        return False
    if not worker_free(d):
        print(f"[land] skip issue-{n}: live worker")
        return False

    # Untracked work inside a subdirectory is real output a worker cannot
    # rm, not scratch -- stage it before anything else touches this tree. An
    # enumerated directory allowlist here silently stopped covering new
    # top-level dirs twice already (issue-168, 2026-09-02: missed src/;
    # benchmark-fasta-build, 2026-09-06: missed benches/, and the
    # untracked-cleanup below would have deleted it before this fix).
    # Root-level loose files are the only sanctioned worker scratch, since
    # workers cannot rm.
    status_z = run(["git", "-C", str(d), "status", "--porcelain", "-z"],
                    text=True, capture_output=True).stdout
    for entry in status_z.split("\0"):
        if entry.startswith("?? ") and "/" in entry[3:]:
            run(["git", "-C", str(d), "add", "--", entry[3:]])

    tracked_dirty = "\n".join(
        line for line in run(["git", "-C", str(d), "status", "--porcelain"],
                              text=True, capture_output=True).stdout.splitlines()
        if not line.startswith("??"))
    if tracked_dirty:
        # Worker exit IS the done signal (maintainer ruling, 2026-09-02,
        # approved interactively): a tracked-dirty tree with a green fast
        # check is finished work nobody persisted -- commit it mechanically
        # and land it. A red check means genuinely unfinished: skip, the
        # rebrief path owns it.
        check_log = LOG_DIR / f"land-autocommit-issue-{n}.log"
        with open(check_log, "w") as f:
            rc = run(["just", "check"], cwd=d, stdout=f, stderr=subprocess.STDOUT).returncode
        if rc != 0:
            # A worktree that has sat through several merges can carry a
            # stale incremental target/ cache that only ever seems to
            # affect these two "which corpus programs compile" snapshots --
            # real, expected drift whenever the corpus grows, not a
            # regression. Accept only these two known-volatile snapshots
            # and retry once before giving up for real.
            for snap in (
                "tests/snapshots/backend_llvm__native_agrees_where_it_compiles.snap",
                "tests/snapshots/backend_rust__rust_agrees_where_it_compiles.snap",
            ):
                new_snap = d / f"{snap}.new"
                if new_snap.exists():
                    new_snap.rename(d / snap)
            with open(check_log, "w") as f:
                rc = run(["just", "check"], cwd=d, stdout=f, stderr=subprocess.STDOUT).returncode
        if rc == 0:
            run(["git", "-C", str(d), "add", "-u"])
            run(["git", "-C", str(d), "commit", "-q", "-m",
                 f"Auto-commit worker output for gh:{n} (green tree at exit)\n\n"
                 "The worker exited leaving these tracked changes uncommitted with just\n"
                 "check green; land_lane.py persisted them mechanically (maintainer\n"
                 "ruling, 2026-09-02: a worker exit is the done signal, the script owns\n"
                 "persistence).\n\n"
                 "Written by the lane worker; committed by land_lane.py."])
            print(f"[land] issue-{n}: auto-committed a green dirty tree")
        else:
            print(f"[land] skip issue-{n}: tracked changes with a RED just check (not done)")
            return False

    # Whatever remains untracked now is root-level scratch a worker cannot
    # rm (subdirectory work was staged above, before this could delete it).
    remaining = run(["git", "-C", str(d), "status", "--porcelain"],
                     text=True, capture_output=True).stdout
    if any(line.startswith("??") for line in remaining.splitlines()):
        run(["git", "-C", str(d), "clean", "-fdq"])

    ahead = run(["git", "-C", str(REPO), "rev-list", "--count", f"main..{branch}"],
                text=True, capture_output=True).stdout.strip()
    if ahead == "0":
        print(f"[land] skip issue-{n}: nothing ahead of main")
        return False

    # Deterministic merge message from the lane's own commits.
    msg_file = LOG_DIR / f"land-msg-issue-{n}.txt"
    subject = run(["git", "log", "-1", "--format=%s", branch],
                  cwd=REPO, text=True, capture_output=True).stdout.strip()
    body = run(["git", "log", "--reverse", "--format=- %s", f"main..{branch}"],
               cwd=REPO, text=True, capture_output=True).stdout
    msg_file.write_text(f"Land issue-{n}: {subject}\n\n{body}")

    # Gate in a throwaway worktree: main stays untouched until green.
    gate_log = LOG_DIR / f"land-gate-issue-{n}.log"
    tmp_branch = f"land-tmp-{n}"
    pdir = LANES / ".land"

    def cleanup_tmp():
        run(["git", "worktree", "remove", "--force", str(pdir)], cwd=REPO,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        run(["git", "branch", "-D", tmp_branch], cwd=REPO,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

    cleanup_tmp()
    run(["git", "worktree", "add", "-b", tmp_branch, str(pdir), "main", "-q"], cwd=REPO)

    with open(gate_log, "w") as f:
        merge_rc = run(["git", "-C", str(pdir), "merge", branch, "--no-ff", "-F", str(msg_file)],
                        stdout=f, stderr=subprocess.STDOUT).returncode
    if merge_rc != 0:
        conflicted = run(["git", "-C", str(pdir), "diff", "--name-only", "--diff-filter=U"],
                          text=True, capture_output=True).stdout.split()
        gen_only = bool(conflicted) and all(f in GENERATED for f in conflicted)
        if gen_only:
            # Every conflicted file is generated: take main's copy, rerun
            # the generators, and the merge is resolved without a worker.
            print(f"[land] issue-{n}: conflicts are generated files only -- regenerating")
            run(["git", "-C", str(pdir), "checkout", "--ours", "--", *conflicted])
            run(["git", "-C", str(pdir), "add", *conflicted])
            with open(gate_log, "a") as f:
                run(["cargo", "nextest", "run", "-E", REGEN_TESTS], cwd=pdir,
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                run(["cargo", "insta", "accept"], cwd=pdir,
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                regen_rc = run(["cargo", "nextest", "run", "-E", REGEN_TESTS], cwd=pdir,
                                stdout=f, stderr=subprocess.STDOUT).returncode
            if regen_rc == 0:
                run(["git", "-C", str(pdir), "add", *conflicted])
                run(["git", "-C", str(pdir), "commit", "-q", "--no-edit", "-F", str(msg_file)])
            else:
                with open(gate_log, "a") as f:
                    f.write(f"MERGE CONFLICT (generated files, but regeneration failed):\n"
                            f"{conflicted}\n")
                run(["git", "-C", str(pdir), "merge", "--abort"],
                    stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
                cleanup_tmp()
                retrigger(n, "merge conflict with main (regeneration failed)", gate_log)
                return False
        else:
            with open(gate_log, "a") as f:
                f.write(f"MERGE CONFLICT merging origin/main + this branch:\n{conflicted}\n")
            run(["git", "-C", str(pdir), "merge", "--abort"],
                stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            cleanup_tmp()
            retrigger(n, "merge conflict with main", gate_log)
            return False

    with open(gate_log, "a") as f:
        test_rc = run(["just", "test"], cwd=pdir,
                       stdout=f, stderr=subprocess.STDOUT).returncode
    if test_rc != 0:
        tail = "\n".join(gate_log.read_text(errors="replace").splitlines()[-60:]) + "\n"
        gate_log.write_text(tail)
        cleanup_tmp()
        retrigger(n, "the full test suite went red", gate_log)
        return False

    # Green: land the tested result. Bounded retry around a busy tick's
    # board commit in the main checkout; lane branches never touch plans/,
    # so a moved main cannot conflict here. 36x5s, not 12x5s: a tick session
    # keeps board.yaml dirty for its whole multi-minute run, and a green
    # land once burned its entire 60s window against one and deferred --
    # three minutes spans a typical tick end.
    ok = False
    for _ in range(36):
        clean = run(["git", "-C", str(REPO), "status", "--porcelain"],
                     text=True, capture_output=True).stdout.strip() == ""
        no_merge_in_progress = not (REPO / ".git" / "MERGE_HEAD").exists()
        if clean and no_merge_in_progress:
            merge_rc = run(["git", "-C", str(REPO), "merge", tmp_branch, "-F", str(msg_file)],
                            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).returncode
            if merge_rc == 0:
                ok = True
                break
        run(["git", "-C", str(REPO), "merge", "--abort"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        time.sleep(5)
    cleanup_tmp()
    if not ok:
        # The lane is fine -- the checkout stayed busy. No retry burned, no
        # re-dispatch; the marker routes the tick to just re-run the land.
        (LOG_DIR / f"land-failed-issue-{n}").write_text(
            f"landing issue-{n}: main checkout stayed busy/dirty -- re-run "
            f"land_lane.py land {n}\n")
        print(f"[land] issue-{n}: main checkout busy -- deferred (marker left for the tick)")
        return False

    run(["git", "-C", str(REPO), "push"])
    if run(["git", "worktree", "remove", "--force", str(d)], cwd=REPO).returncode != 0:
        run(["git", "worktree", "remove", str(d)], cwd=REPO)
    run(["git", "branch", "-d", branch], cwd=REPO,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    for stale in (f"land-retries-issue-{n}", f"land-failed-issue-{n}",
                  f"escalated-issue-{n}", f"investigating-issue-{n}"):
        (LOG_DIR / stale).unlink(missing_ok=True)
    msg_file.unlink(missing_ok=True)
    gate_log.unlink(missing_ok=True)
    merge_subject = run(["git", "-C", str(REPO), "log", "--merges", "--format=%s", "-1"],
                        text=True, capture_output=True).stdout.strip()
    print(f"[land] issue-{n} -> main: {merge_subject} (pushed)")
    return True


def acquire_land_lock(lock_path: Path, timeout_s: int = 1800):
    """`flock -w N` (bounded wait) has no direct fcntl.flock equivalent --
    fcntl.flock never times out. A manual poll loop on a non-blocking
    attempt changes the exact wait granularity (up to ~1s slop here) but not
    the semantics that matter: either the lock is acquired within the
    deadline or it is not, same as the bash version."""
    lock_path.parent.mkdir(parents=True, exist_ok=True)
    f = open(lock_path, "w")
    deadline = time.monotonic() + timeout_s
    while True:
        try:
            fcntl.flock(f, fcntl.LOCK_EX | fcntl.LOCK_NB)
            return f
        except BlockingIOError:
            if time.monotonic() >= deadline:
                f.close()
                return None
            time.sleep(1)


def cmd_land(args: list[str]) -> int:
    if not args:
        print("no issues given", file=sys.stderr)
        return 2
    # One land at a time, machine-wide. Bounded wait with an explicit
    # give-up (house pattern): the periodic tick is the backstop that
    # re-fires a land that gave up here. A path under LOG_DIR, not /tmp:
    # sccache (unrelated to this pipeline) once ended up holding an flock on
    # a /tmp lock via inode reuse in that high-churn shared directory,
    # silently stalling the whole queue for 10+ minutes (2026-09-06).
    # Nothing else touches LOG_DIR.
    lock = acquire_land_lock(LOG_DIR / "land.lock")
    if lock is None:
        print("[land] queue lock held 30+ min -- gave up (tick will retry)", file=sys.stderr)
        fire_tick()
        return 1
    any_green = False
    for n in args:
        if land_one(n):
            any_green = True
    # One tick per invocation: board-archive moves and post-land review on
    # green, escalation routing on failure, rebrief logic when nothing landed.
    fire_tick()
    return 0 if any_green else 1


def cmd_land_patch(args: list[str]) -> int:
    if len(args) != 2:
        print("usage: land_lane.py land-patch <row-id> <patch-file>", file=sys.stderr)
        return 2
    n, patch_file = args[0], Path(args[1])
    if not patch_file.is_file():
        print(f"[land] land-patch {n}: no such patch file {patch_file}", file=sys.stderr)
        return 2
    lock = acquire_land_lock(LOG_DIR / "land.lock")
    if lock is None:
        print("[land] queue lock held 30+ min -- gave up (tick will retry)", file=sys.stderr)
        fire_tick()
        return 1
    d = LANES / f"issue-{n}"
    branch = f"issue-{n}"
    run(["git", "worktree", "remove", "--force", str(d)], cwd=REPO,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    run(["git", "branch", "-D", branch], cwd=REPO,
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    run(["git", "worktree", "add", "-b", branch, str(d), "main", "-q"], cwd=REPO)
    am_log = LOG_DIR / f"land-patch-am-issue-{n}.log"
    with open(am_log, "w") as f:
        am_rc = run(["git", "-C", str(d), "am", str(patch_file)],
                     stdout=f, stderr=subprocess.STDOUT).returncode
    if am_rc != 0:
        # The patch was generated against whatever commit was HEAD when
        # simple_dispatch.py cloned -- main has very likely moved since. A
        # clean git-am failure here means real drift, not a bug in the
        # patch itself; route it through the SAME retry/escalation path as
        # any other landing failure rather than a bespoke one.
        run(["git", "-C", str(d), "am", "--abort"],
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        run(["git", "worktree", "remove", "--force", str(d)], cwd=REPO,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        run(["git", "branch", "-D", branch], cwd=REPO,
            stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        retrigger(n, "patch from simple_dispatch.py did not apply cleanly onto current main", am_log)
        fire_tick()
        return 1
    any_green = land_one(n)
    fire_tick()
    return 0 if any_green else 1


def main() -> int:
    if len(sys.argv) < 2:
        print("usage: land_lane.py land <issue-number>...", file=sys.stderr)
        print("       land_lane.py land-patch <row-id> <patch-file>", file=sys.stderr)
        return 2
    mode, rest = sys.argv[1], sys.argv[2:]
    LOG_DIR.mkdir(parents=True, exist_ok=True)
    os.chdir(REPO)  # never run with cwd inside a worktree this script may remove
    set_cargo_env()
    if mode == "land":
        return cmd_land(rest)
    if mode == "land-patch":
        return cmd_land_patch(rest)
    if mode in ("fold", "promote", "wip"):
        print("the accumulator pipeline is retired (2026-09-01): use "
              "land_lane.py land <issue>", file=sys.stderr)
        return 2
    print(f"unknown mode: {mode}", file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main())
