#!/usr/bin/env python3
"""Dispatch a toylang board task to a disposable, fully-permissive microsandbox
microVM, verify it with the real toolchain (just check), and retry with the
exact failure evidence (same opencode session, same sandbox) until green or a
retry cap is hit. Never touches the real lane or main -- land-lane.sh's own
`just test` gate stays the final authority; this is a fast pre-filter that
runs before a lane is ever proposed for landing.

Lessons this design bakes in from the manual spike that preceded it:
  - opencode's `run` hangs forever on non-TTY stdin with no EOF: every
    invocation redirects stdin from /dev/null (see run_opencode()).
  - A sandbox whose entrypoint IS the task stops when the task exits, and
    /tmp is tmpfs -- wiped on the next msb exec's implicit restart. This
    harness keeps one sandbox alive per attempt with `sleep infinity` as
    the entrypoint and does everything else via `msb exec`, writing any
    file it needs to survive to /root (real disk), never /tmp.
  - `--secret ENV@HOST` keeps the API key out of the guest entirely; the
    key must also be present as a host env var on every `msb exec` call
    against a --secret sandbox, not just at boot.
  - Continuing the SAME opencode session on retry (--continue) means the
    model already has its own exploration in context instead of re-reading
    every file from scratch -- the single most effective lever found for
    reducing DeepSeek V4 Flash's context-heavy-task budget burn.

Usage:
  sandbox_dispatch.py <board-row-id> --brief path/to/brief.txt
      [--model openrouter/deepseek/deepseek-v4-flash-0731]
      [--retry-cap 2] [--snapshot toylang-toolchain] [--keep-sandbox]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path

REPO = Path("/home/kantord/repos/toylang")
LANES = Path.home() / ".local/share/toylang-lanes"
AUTH_JSON = Path.home() / ".local/share/opencode/auth.json"
MSB_BIN = Path.home() / ".local/bin/msb"
DEFAULT_MODEL = "openrouter/deepseek/deepseek-v4-flash-0731"
DEFAULT_PLAN_MODEL = "openrouter/z-ai/glm-5.2"
DEFAULT_SNAPSHOT = "toylang-toolchain"
TOOLCHAIN_PATH_EXPORT = (
    "export PATH=$HOME/.cargo/bin:/usr/lib/llvm-22/bin:$PATH"
)

PLAN_PROMPT_TEMPLATE = """Before writing or editing anything, evaluate this task's shape.

TASK:
{task}

Decide one of three verdicts:
- "trivial": a single build session can implement the whole task and reach a green `just check`.
- "refactor-first": the real change touches many call sites that share an awkward, error-prone
  shape (e.g. deeply nested constructor calls where parenthesis-counting mistakes are likely,
  or a pattern repeated across many backends that a small shared helper would simplify), and a
  small, separate preparatory refactor would make the real change significantly easier and less
  error-prone to get right. Scope the refactor tightly -- it must NOT implement any part of the
  real feature, only reshape existing code to make the coming change easier.
- "split": the task is really two or more independent sub-tasks (e.g. per-backend work that does
  not depend on the others) that can each be implemented and verified on their own.

You have {rounds_left} more round(s) of this evaluate-then-refactor cycle available after this one
before the harness will just attempt the full task directly -- that direct attempt is always a
safe fallback, so if you are not confident a decomposition fits in the remaining rounds, pick
"trivial" rather than starting a decomposition you cannot finish.

Read whatever source you need to make this judgment, but do NOT write or edit any file except the
verdict file itself.

Write your decision to /root/verdict.json as a single JSON object, exactly one of these shapes:
{{"verdict": "trivial", "reasoning": "one paragraph"}}
{{"verdict": "refactor-first", "reasoning": "...", "refactor_brief": "the exact small change to make now, with its own definition of done -- just the refactor, not the feature"}}
{{"verdict": "split", "reasoning": "...", "split_briefs": ["complete self-contained brief for sub-task 1", "complete self-contained brief for sub-task 2", "..."]}}

Use your write tool to create /root/verdict.json with valid JSON (no trailing commas, no comments),
then stop. Do not implement anything yet.
"""

DEVILS_ADVOCATE_PROMPT_TEMPLATE = """You are reviewing a planning decision, not writing code. Do NOT
read any files or explore the repository -- judge this purely on the text below. This should be a
fast, cheap review, not a re-investigation.

ORIGINAL TASK (for scope only):
{task}

THE PLANNER'S VERDICT:
{verdict_json}

Is this verdict actually justified given the task's real scope? Look specifically for a mismatch
between the claimed verdict and the stated facts -- e.g. "trivial" claimed for something that
admits it touches many files/backends/modules, or a "refactor-first" whose refactor_brief secretly
implements real feature work, or a "split" whose pieces are not actually independent. A verdict
that leans on a prior/reference implementation is not automatically safe if that reference is
described as unreliable, abandoned, or needing re-derivation.

Write your review to /root/critique.json as a single JSON object:
{{"agree": true}}
or
{{"agree": false, "objection": "one paragraph: specifically what fact in the verdict does not
support its conclusion, and what verdict would fit the stated facts better"}}

Use your write tool to create /root/critique.json, then stop.
"""

BUILD_AFTER_TRIVIAL = (
    "Your plan-phase evaluation judged this task trivial for one build session. "
    "Implement it now, to the full definition of done in the original brief above."
)

BUILD_AFTER_DECOMPOSE = (
    "The decomposition above is complete. Implement whatever of the original task remains "
    "(if anything), then verify the whole thing reaches the full definition of done in the "
    "original brief."
)


@dataclass
class Attempt:
    n: int
    verify_ok: bool
    verify_tail: str


def sh(cmd: list[str], env: dict | None = None, check: bool = True,
       capture: bool = True) -> subprocess.CompletedProcess:
    print(f"$ {' '.join(cmd)}", file=sys.stderr)
    r = subprocess.run(cmd, env=env, check=False, text=True, capture_output=capture)
    if r.returncode != 0:
        print(f"  rc={r.returncode} stderr={r.stderr!r}", file=sys.stderr)
        if check:
            r.check_returncode()
    return r


def msb_env() -> dict:
    env = os.environ.copy()
    env["PATH"] = f"{Path.home() / '.local/bin'}:{env.get('PATH', '')}"
    key = json.loads(AUTH_JSON.read_text())["openrouter"]["key"]
    env["OPENROUTER_API_KEY"] = key
    return env


def prepare_clone(issue_id: str, workdir: Path) -> tuple[Path, str]:
    """Disposable local clone at the lane's current state (or main's tip if
    no lane worktree exists yet). Never touches the real lane or main repo."""
    clone_dir = workdir / "repo"
    if clone_dir.exists():
        shutil.rmtree(clone_dir)
    sh(["git", "clone", "--no-hardlinks", "--quiet", str(REPO), str(clone_dir)])
    branch = f"issue-{issue_id}"
    # `git clone` of a local repo only checks out the DEFAULT branch locally;
    # every other branch (including lane branches, which are never pushed to
    # a remote) lands as an `origin/<branch>` remote-tracking ref, not a
    # plain local branch. Checking `rev-parse --verify <branch>` directly
    # against a fresh clone always misses this and silently branches off
    # main's current tip instead -- confirmed the hard way (2026-09-05): a
    # test run for issue-172 branched off main AFTER an unrelated same-day
    # merge, completely disconnected from that lane's real history.
    remote_ref = f"origin/{branch}"
    have_branch = sh(["git", "-C", str(clone_dir), "rev-parse", "--verify", remote_ref],
                      check=False).returncode == 0
    if have_branch:
        sh(["git", "-C", str(clone_dir), "checkout", "--quiet", "-b", branch, remote_ref])
    else:
        sh(["git", "-C", str(clone_dir), "checkout", "--quiet", "-b", branch])
    base_commit = sh(["git", "-C", str(clone_dir), "rev-parse", "HEAD"]).stdout.strip()

    lane = LANES / f"issue-{issue_id}"
    if lane.is_dir():
        diff = sh(["git", "-C", str(lane), "diff"]).stdout
        if diff.strip():
            patch = workdir / "lane.patch"
            patch.write_text(diff)
            sh(["git", "apply", str(patch)], check=False)  # best-effort; harness still proceeds if it doesn't apply
    return clone_dir, base_commit


def write_permissive_config(workdir: Path) -> Path:
    cfg = workdir / "opencode.jsonc"
    cfg.write_text(json.dumps({
        "$schema": "https://opencode.ai/config.json",
        # Disposable microVM: blast radius is the VM, not the host. No
        # hand-curated allow-list needed -- that is the point of the sandbox.
        "permission": {
            "edit": "allow",
            "webfetch": "allow",
            "external_directory": "allow",
            "bash": {"*": "allow"},
        },
    }))
    return cfg


def opencode_binary() -> Path:
    for candidate in (Path("/usr/bin/opencode"),):
        if candidate.exists():
            return candidate
    raise FileNotFoundError("opencode binary not found; expected /usr/bin/opencode")


def boot_sandbox(name: str, clone_dir: Path, cfg_path: Path, opencode_bin: Path,
                  snapshot: str, env: dict) -> None:
    # --copy-dir/--copy-file ("patches") cannot combine with --from-snapshot
    # (a snapshot pins the whole filesystem state at boot) -- confirmed by
    # `error: invalid config: patches cannot be combined with from_snapshot`.
    # So the sandbox boots clean from the snapshot with just an idle
    # entrypoint, and every file (repo clone, opencode binary, config)
    # arrives afterward via `msb copy`, which works fine post-boot.
    # 4G crashed the linker with a Bus Error (SIGBUS) mid-`just check`
    # (2026-09-05): rust-lld linking inkwell/LLVM-22 test binaries, several
    # in parallel under nextest, ran the tmpfs-backed guest out of memory.
    # 12G gives real headroom; CARGO_BUILD_JOBS in verify() further caps
    # concurrent linker processes.
    # The toolchain snapshot itself now has a 40G root disk baked in (the
    # real fix for repeated "sandbox fs error: flush: No space left on
    # device" crashes, 2026-09-05 -- not a memory issue, ruled out at 16G
    # RAM). `--root-disk` cannot be passed here: it requires a plain OCI
    # image and is rejected outright when combined with --from-snapshot.
    args = [str(MSB_BIN), "run", "-m", "16G", "-c", "4", "--no-tty", "-d",
            "--name", name, "--replace",
            "--secret", "OPENROUTER_API_KEY@openrouter.ai"]
    from_snapshot = sh([str(MSB_BIN), "snapshot", "list"], check=False).stdout
    if snapshot in from_snapshot:
        args += ["--from-snapshot", snapshot]
    else:
        print(f"warning: snapshot '{snapshot}' not found, booting bare debian "
              "(just check will fail without the toolchain)", file=sys.stderr)
        args += ["debian"]
    args += ["--", "sh", "-c", "sleep infinity"]
    sh(args, env=env)
    time.sleep(2)

    sh([str(MSB_BIN), "copy", str(clone_dir), f"{name}:/repo"], env=env)
    sh([str(MSB_BIN), "copy", str(opencode_bin), f"{name}:/usr/local/bin/opencode"], env=env)
    exec_in(name, "mkdir -p /root/.config/opencode", env)
    sh([str(MSB_BIN), "copy", str(cfg_path), f"{name}:/root/.config/opencode/opencode.jsonc"], env=env)
    exec_in(name, "chmod +x /usr/local/bin/opencode && cd /repo && "
                  "git config user.name 'Daniel Kantor' && "
                  "git config user.email 'git@daniel-kantor.com'", env)


def exec_in(name: str, script: str, env: dict, check: bool = True) -> subprocess.CompletedProcess:
    return sh([str(MSB_BIN), "exec", name, "--", "sh", "-c", script], env=env, check=check)


def send_text(name: str, guest_path: str, text: str, workdir: Path, env: dict, tag: str) -> None:
    """Deliver text into the guest via `msb copy`, not a heredoc through `sh
    -c`. A heredoc containing a backtick-wrapped phrase (`` `just check` ``)
    reproducibly hung `msb exec` for over two minutes on a trivial sandbox
    with nothing else running (2026-09-05) -- root cause not chased down
    since `msb copy` sidesteps the whole class of shell-quoting risk and is
    already proven reliable for exactly this."""
    local = workdir / f"{tag}.txt"
    local.write_text(text)
    sh([str(MSB_BIN), "copy", str(local), f"{name}:{guest_path}"], env=env)


def run_opencode(name: str, message_file_guest: str, model: str, env: dict,
                  continue_session: bool, agent: str | None = None) -> str:
    """One opencode turn. `< /dev/null` is load-bearing: opencode run hangs
    forever on non-TTY stdin with no EOF (confirmed root cause, 2026-09-05).
    `agent`: None lets opencode default to "build"; pass "plan" for a
    read-only evaluation turn (plan is restrictive by default -- edit/bash
    ask -- but our own opencode.jsonc already blanket-allows everything, so
    this only affects which system prompt/model config opencode selects).
    """
    cont = "--continue " if continue_session else ""
    agent_flag = f"--agent {agent} " if agent else ""
    script = (
        f"cd /repo && "
        f"timeout --kill-after=30s 1800s opencode run {cont}{agent_flag}"
        f'"$(cat {message_file_guest})" -m {model} < /dev/null '
        f"> /root/opencode-run.log 2>&1; "
        f"echo RC=$? >> /root/opencode-run.log"
    )
    exec_in(name, script, env, check=False)
    tail = exec_in(name, "tail -c 4000 /root/opencode-run.log", env, check=False).stdout
    return tail


def read_json_from_guest(name: str, guest_path: str, env: dict) -> dict | None:
    r = exec_in(name, f"cat {guest_path} 2>/dev/null", env, check=False)
    if not r.stdout.strip():
        return None
    try:
        return json.loads(r.stdout)
    except json.JSONDecodeError as e:
        print(f"  warning: {guest_path} is not valid JSON: {e}", file=sys.stderr)
        return None


def plan_phase(name: str, task_text: str, plan_model: str, env: dict, workdir: Path,
               round_no: int, rounds_left: int) -> dict | None:
    """One plan-phase turn: dispatch --agent plan on the plan model, asking
    for a structured trivial/refactor-first/split verdict instead of code.
    Continues the same session from round 2 onward so the model keeps its
    own prior exploration instead of re-reading the codebase from scratch."""
    prompt = PLAN_PROMPT_TEMPLATE.format(task=task_text, rounds_left=rounds_left)
    guest_path = f"/root/plan-{round_no}.txt"
    send_text(name, guest_path, prompt, workdir, env, f"plan-{round_no}-sent")
    exec_in(name, "rm -f /root/verdict.json", env, check=False)
    run_opencode(name, guest_path, plan_model, env,
                 continue_session=(round_no > 0), agent="plan")
    return read_json_from_guest(name, "/root/verdict.json", env)


def devils_advocate_phase(name: str, task_text: str, verdict: dict, critic_model: str,
                           env: dict, workdir: Path, round_no: int) -> dict | None:
    """A cheap, FRESH (no --continue) session that only sees the task's
    stated scope and the planner's verdict+reasoning -- never the planner's
    own exploration. Cheaper by construction: no codebase re-reading, just a
    consistency check of a claim against stated facts. Uses the cheap build
    model, not the plan model -- this is a small-input judgment task, not a
    context-heavy one, so DeepSeek's weakness (burning budget re-deriving
    context on LARGE tasks) doesn't apply here."""
    prompt = DEVILS_ADVOCATE_PROMPT_TEMPLATE.format(
        task=task_text, verdict_json=json.dumps(verdict, indent=2))
    guest_path = f"/root/critic-{round_no}.txt"
    send_text(name, guest_path, prompt, workdir, env, f"critic-{round_no}-sent")
    exec_in(name, "rm -f /root/critique.json", env, check=False)
    run_opencode(name, guest_path, critic_model, env, continue_session=False, agent="plan")
    return read_json_from_guest(name, "/root/critique.json", env)


def verify(name: str, env: dict) -> tuple[bool, str]:
    # CARGO_BUILD_JOBS=2: cap concurrent rustc/lld processes -- several
    # linking the full inkwell/LLVM-22 chain at once is what caused the
    # SIGBUS at -m 4G; even at -m 12G this keeps peak memory well clear.
    script = (f"cd /repo && {TOOLCHAIN_PATH_EXPORT} && export CARGO_BUILD_JOBS=2 && "
              f"just check > /root/check.log 2>&1; echo RC=$? >> /root/check.log; tail -c 6000 /root/check.log")
    r = exec_in(name, script, env, check=False)
    out = r.stdout
    ok = "\nRC=0" in out or out.strip().endswith("RC=0")
    return ok, out


def ensure_committed(name: str, env: dict) -> None:
    """A green (or final) tree with uncommitted tracked changes is still done
    work nobody persisted (land-lane.sh's own rule for the same situation) --
    commit it mechanically rather than losing it to a missed commit step."""
    exec_in(
        name,
        "cd /repo && "
        "if [ -n \"$(git status --porcelain | grep -v '^??')\" ]; then "
        "git add -u && git add -- src tests docs site plans 2>/dev/null; "
        "git commit -q -m 'Auto-commit sandbox worker output (tracked changes at verify time)' "
        "|| true; fi",
        env,
        check=False,
    )


def extract_result(name: str, base_commit: str, env: dict, out_dir: Path) -> Path | None:
    ensure_committed(name, env)
    exec_in(name, f"cd /repo && rm -f /root/*.patch; "
                  f"git format-patch {base_commit} -o /root/ >/root/format-patch.log 2>&1",
            env, check=False)
    r = exec_in(name, "cat /root/*.patch 2>/dev/null", env, check=False)
    if not r.stdout.strip():
        return None
    out = out_dir / "result.patch"
    out.write_text(r.stdout)
    return out


def run_build_cycle(issue_id: str, name: str, first_message_guest: str, model: str,
                     env: dict, workdir: Path, retry_cap: int, base_commit: str,
                     continue_session: bool) -> tuple[list[Attempt], Path | None]:
    """Dispatch + verify + retry-with-evidence until green or retry_cap is
    hit. This is the harness's original (pre-decomposition) loop, now reused
    as the "actually build it" tail end of the plan-decompose flow."""
    attempts: list[Attempt] = []
    result_patch: Path | None = None
    for i in range(retry_cap + 1):
        print(f"== {issue_id}: build turn {i + 1} ==", file=sys.stderr)
        if i == 0:
            run_opencode(name, first_message_guest, model, env, continue_session=continue_session)
        else:
            fb_guest = f"/root/feedback-{i}.txt"
            feedback = (
                "The verification gate failed after your last change. Fix the specific "
                f"failures below -- do not start over or redo work that already passed.\n\n{attempts[-1].verify_tail}"
            )
            send_text(name, fb_guest, feedback, workdir, env, f"feedback-{i}-sent")
            run_opencode(name, fb_guest, model, env, continue_session=True)

        print(f"== {issue_id}: verifying build turn {i + 1} ==", file=sys.stderr)
        ok, tail = verify(name, env)
        attempts.append(Attempt(i + 1, ok, tail))
        print(tail[-2000:], file=sys.stderr)

        if ok:
            print(f"== {issue_id}: GREEN on attempt {i + 1} ==", file=sys.stderr)
            result_patch = extract_result(name, base_commit, env, workdir)
            return attempts, result_patch
        print(f"== {issue_id}: red on attempt {i + 1}, "
              f"{'retrying' if i < retry_cap else 'cap reached'} ==", file=sys.stderr)
    result_patch = extract_result(name, base_commit, env, workdir)
    return attempts, result_patch


def run_plan_decompose(issue_id: str, name: str, task_text: str, plan_model: str,
                        build_model: str, critic_model: str, env: dict, workdir: Path,
                        base_commit: str, max_plan_rounds: int) -> tuple[str, bool]:
    """Runs up to max_plan_rounds of evaluate-then-refactor before the real
    build attempt. Returns (next_build_message_guest_path, session_already_started).
    Every dispatch here continues the SAME session, so the eventual build
    phase inherits all of this exploration/refactor context for free."""
    started = False
    for round_no in range(max_plan_rounds):
        rounds_left = max_plan_rounds - round_no - 1
        print(f"== {issue_id}: plan round {round_no + 1}/{max_plan_rounds} "
              f"({rounds_left} left after this) ==", file=sys.stderr)
        verdict = plan_phase(name, task_text, plan_model, env, workdir, round_no, rounds_left)
        started = True
        if verdict is None:
            print(f"== {issue_id}: plan round {round_no + 1} produced no verdict.json, "
                  "falling back to trivial ==", file=sys.stderr)
            break

        # Devil's advocate: a fresh, uncontexted, cheap-model review of the
        # verdict's stated justification against the task's stated scope --
        # not a re-exploration. If it objects, send the objection back to
        # the SAME plan session (which still has full context) for one
        # defend-or-revise round, then use whatever verdict.json holds now.
        # Capped at one correction per plan round for the same reason the
        # plan phase itself is round-capped: a safe fallback (use the
        # original verdict) beats an unbounded back-and-forth.
        critique = devils_advocate_phase(name, task_text, verdict, critic_model,
                                          env, workdir, round_no)
        if critique and critique.get("agree") is False:
            objection = critique.get("objection", "(no objection text given)")
            print(f"== {issue_id}: devil's advocate objects: {objection[:300]} ==",
                  file=sys.stderr)
            correction_guest = f"/root/correction-{round_no}.txt"
            send_text(
                name, correction_guest,
                f"A reviewer raised this objection to your verdict: {objection}\n\n"
                "Defend your original verdict with a rebuttal, or revise it -- either way, "
                "rewrite /root/verdict.json with your final decision now.",
                workdir, env, f"correction-{round_no}-sent")
            exec_in(name, "rm -f /root/verdict.json", env, check=False)
            run_opencode(name, correction_guest, plan_model, env,
                         continue_session=True, agent="plan")
            revised = read_json_from_guest(name, "/root/verdict.json", env)
            if revised is not None:
                verdict = revised
        else:
            print(f"== {issue_id}: devil's advocate agrees ==", file=sys.stderr)

        kind = verdict.get("verdict")
        print(f"== {issue_id}: verdict = {kind} -- {verdict.get('reasoning', '')[:300]} ==",
              file=sys.stderr)

        if kind == "trivial":
            break

        if kind == "refactor-first" and verdict.get("refactor_brief"):
            rf_guest = f"/root/refactor-{round_no}.txt"
            send_text(name, rf_guest, verdict["refactor_brief"], workdir, env, f"refactor-{round_no}-sent")
            run_opencode(name, rf_guest, build_model, env, continue_session=True, agent="build")
            ok, tail = verify(name, env)
            print(f"== {issue_id}: refactor round {round_no + 1} verify: "
                  f"{'green' if ok else 'red'} ==", file=sys.stderr)
            ensure_committed(name, env)
            continue  # re-evaluate the (hopefully now simpler) remaining task

        if kind == "split" and verdict.get("split_briefs"):
            for j, sub_brief in enumerate(verdict["split_briefs"]):
                sub_guest = f"/root/split-{round_no}-{j}.txt"
                send_text(name, sub_guest, sub_brief, workdir, env, f"split-{round_no}-{j}-sent")
                run_opencode(name, sub_guest, build_model, env, continue_session=True, agent="build")
                verify(name, env)  # best-effort per-subtask check; final cycle re-verifies everything
                ensure_committed(name, env)
            break

        # Unrecognized/malformed verdict shape -- don't loop forever on garbage.
        print(f"== {issue_id}: unrecognized verdict shape, treating as trivial ==", file=sys.stderr)
        break

    final_guest = "/root/build-final.txt"
    send_text(name, final_guest, BUILD_AFTER_DECOMPOSE if started else task_text,
              workdir, env, "build-final-sent")
    return final_guest, started


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                  formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("issue_id", help="board row id / lane slug, e.g. param-destructure-build")
    ap.add_argument("--brief", required=True, type=Path)
    ap.add_argument("--model", default=DEFAULT_MODEL)
    ap.add_argument("--plan-model", default=DEFAULT_PLAN_MODEL)
    ap.add_argument("--critic-model", default=DEFAULT_MODEL,
                     help="devil's-advocate reviewer of the plan verdict -- cheap by design, "
                          "since it never inherits context, only the stated verdict + task scope")
    ap.add_argument("--max-plan-rounds", type=int, default=2,
                     help="0 disables the plan-decompose phase entirely")
    ap.add_argument("--retry-cap", type=int, default=2)
    ap.add_argument("--snapshot", default=DEFAULT_SNAPSHOT)
    ap.add_argument("--keep-sandbox", action="store_true",
                     help="don't remove the sandbox on exit (for debugging)")
    ap.add_argument("--workdir", type=Path, default=None)
    args = ap.parse_args()

    workdir = args.workdir or Path(f"/tmp/sandbox-dispatch-{args.issue_id}")
    workdir.mkdir(parents=True, exist_ok=True)
    name = f"sd-{args.issue_id}"[:32]

    print(f"== {args.issue_id}: preparing disposable clone ==", file=sys.stderr)
    clone_dir, base_commit = prepare_clone(args.issue_id, workdir)
    cfg_path = write_permissive_config(workdir)
    opencode_bin = opencode_binary()
    env = msb_env()

    print(f"== {args.issue_id}: booting sandbox {name} ==", file=sys.stderr)
    boot_sandbox(name, clone_dir, cfg_path, opencode_bin, args.snapshot, env)

    task_text = args.brief.read_text()
    brief_guest = "/root/brief.txt"
    send_text(name, brief_guest, task_text, workdir, env, "brief-sent")

    try:
        if args.max_plan_rounds > 0:
            first_build_guest, session_started = run_plan_decompose(
                args.issue_id, name, task_text, args.plan_model, args.model, args.critic_model,
                env, workdir, base_commit, args.max_plan_rounds)
        else:
            first_build_guest, session_started = brief_guest, False

        attempts, result_patch = run_build_cycle(
            args.issue_id, name, first_build_guest, args.model, env, workdir,
            args.retry_cap, base_commit, continue_session=session_started)
    finally:
        if not args.keep_sandbox:
            sh([str(MSB_BIN), "rm", "-f", name], env=env, check=False)

    summary = {
        "issue_id": args.issue_id,
        "attempts": len(attempts),
        "green": attempts[-1].verify_ok if attempts else False,
        "result_patch": str(result_patch) if result_patch else None,
        "workdir": str(workdir),
    }
    print(json.dumps(summary, indent=2))
    return 0 if summary["green"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
