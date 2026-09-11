# .claude/scripts/ as a proper uv-managed Python project -- plan

Explicit user directive: make the project's tooling scripts (`.claude/scripts/`) a
proper Python project managed with `uv`, with no shell scripts left. This document is
the PLAN, reviewed adversarially before any code changes (per the user's explicit
process: 3 rounds of skeptic review on this plan, then implementation, then 2 more
rounds of skeptic review on the implementation).

## Current state (surveyed directly, not assumed)

`.claude/scripts/` today:

Python already (keep as `.py`, migrate into the new project structure, no logic change
unless a review round finds a real reason to):
- `agent_loop.py`, `simple_dispatch.py`, `dispatch-state.py` -- this session's own work
- `board-archive.py`, `board-lint.py` -- generic board hygiene
- `lane-telemetry.py` -- a live `SessionEnd` hook (`.claude/settings.json`), generic
  session telemetry, unrelated to dispatch
- `opencode-peek.py` -- live-view renderer, used only by `opencode-worker.sh`
- `tick-stream.py` -- colorizes `drive-tick.sh`'s `claude -p --output-format
  stream-json` feed; load-bearing for the coordinator loop

Shell, to be converted to Python:
- `drive-loop.sh` (821 bytes) -- trivial `while true` wrapper firing `drive-tick.sh`
  on an interval, with periodic `audit` ticks
- `drive-tick.sh` (281 lines, `wc -l` re-checked in round 2 -- the plan's own earlier
  "~600" figure was wrong: it was computed from the doc's own draft-authoring time,
  which was actually AFTER commit `a504736` had already cut the file from 383 to 281
  lines removing the old worktree/pgrep/ESCALATION.md archaeology; never accurate) --
  the coordinator's own mechanical preamble: lock acquisition, dev-server revival,
  delegated-row state via `dispatch-state.py`, trigger computation, the `claude -p`
  invocation piped through `tick-stream.py`, coordinator-auth-failure detection
- `land-lane.sh` (353 lines, re-checked in round 2; close to the earlier "~370"
  estimate, not a real error) -- the serial landing queue: flock, worktree/branch
  materialization, the full `just test` gate in a throwaway worktree, generated-file
  conflict auto-resolution, bounded-retry merge into the busy main checkout, push,
  retry/escalation on failure
- `opencode-worker.sh` (113 lines, re-checked in round 2 -- the earlier "~150" was a
  plain miscount, not staleness: the file hasn't changed since 2026-09-06) -- launches
  one `opencode run` turn for the
  explicit visible-kitty-window delegation variant (`enwiro-delegate` skill), pipes
  through `opencode-peek.py`, fires `land-lane.sh land` on exit
- `tick-peek.sh` (443 bytes) -- trivial: tails the coordinator's own tick transcript
  through `tick-stream.py` for a human watching live

Real complexity to preserve, not simplify away by accident: `land-lane.sh` and
`drive-tick.sh` are both the product of many real, individually-cited incidents (fd
leaks, flock stalls, subshell-elision hangs, bash/pgrep race conditions) -- every one
of those comments is a real, previously-hit bug, not decoration. The migration's job
is to preserve every one of those fixes' actual EFFECT in Python, not just their shape
in bash.

## Proposed project structure

- `pyproject.toml` at `.claude/scripts/` (making that directory a real `uv` project
  root, not the repo root -- the repo root's own Rust/Cargo project should not gain an
  unrelated Python project file mixed into it).
- `dependencies = ["pyyaml"]` -- the only real third-party dependency actually used
  today (`board-archive.py`, `board-lint.py`, `dispatch-state.py`'s
  `dispatch_trigger()`). Everything else (`urllib`, `subprocess`, `fcntl`, `json`,
  `csv`, `argparse`) is stdlib.
- `uv.lock` committed alongside, so every invocation resolves identically. `.gitignore`
  needs a `.venv` entry added (round-3 review: confirmed by direct reproduction that
  `uv run --project <dir> <script>` creates `<dir>/.venv` on first run; the repo's
  current `.gitignore` has no Python-related entry at all beyond `__pycache__/`).
- Each script keeps its CURRENT filename, `.sh` -> `.py`, in the SAME directory --
  NOT reorganized into an installable package (`src/toylang_tools/...`) with
  `[project.scripts]` console entries. Reasoning, not just inertia: dozens of existing
  references across skills/docs/scripts hardcode paths like
  `.claude/scripts/drive-tick.sh` and `nohup python3 .claude/scripts/X.py`; keeping
  the same directory and an analogous filename (`drive-tick.sh` -> `drive_tick.py`,
  matching Python's own module-naming convention) means every reference needs a
  mechanical rename, not a structural rewrite, and every script stays trivially
  runnable via `python3 <path>` exactly like today -- `uv` governs the environment
  and dependency resolution, not the invocation shape.
- Invocation: `uv run --project .claude/scripts <path-to-script>.py <args>` from
  anywhere (an absolute `--project` path, resolved once per call site, works
  regardless of caller cwd -- confirmed this is how `uv run` expects to be pointed at
  a non-cwd project). Every current `python3 .claude/scripts/X.py` call site becomes
  `uv run --project /home/kantord/repos/toylang/.claude/scripts
  /home/kantord/repos/toylang/.claude/scripts/X.py` (or a `$SCRIPTS`-relative
  equivalent inside scripts that already carry that variable).
  - **Explicit carve-out (round-3 review, important)**: `agent_loop.py`'s OWN runtime
    invocation must NOT be converted. `simple_dispatch.py` (`msb copy`, then
    `timeout ... python3 /root/agent_loop.py ...`, confirmed at
    `simple_dispatch.py:389,433`) runs it inside a disposable microVM guest that has
    no `uv`, no `.claude/scripts`, no `pyproject.toml` at all -- a mechanical
    find-and-convert-every-`python3 .claude/scripts/X.py`-site sweep would break
    dispatch outright if applied here. Safe only because `agent_loop.py` is
    confirmed stdlib-only (no PyYAML, no third-party imports) so it doesn't actually
    need `uv`'s environment inside the guest. Leave this one invocation as plain
    `python3` on purpose; do not "fix" it during the cross-reference sweep.
  - **`simple_dispatch.py`'s OWN invocation form also needs updating** (round-3
    review) even though its filename isn't changing: it's a bare `python3
    .claude/scripts/simple_dispatch.py` call embedded as literal, copy-executed text
    in three places the coordinator LLM and human operators actually run verbatim --
    `drive-tick.sh`'s own `POLICY` string (line 216, `nohup python3
    .claude/scripts/simple_dispatch.py ROW-ID-1 ROW-ID-2 ROW-ID-3 --brief-dir
    plans/simple-briefs --parallel 3 &`), `.claude/skills/drive/SKILL.md` (the same
    command), and `.claude/skills/enwiro-delegate/SKILL.md` (`nohup python3
    .claude/scripts/simple_dispatch.py <row-id> --brief-dir <dir> &`). These are
    invisible to a "grep for retired filenames" sweep since `simple_dispatch.py`
    isn't being renamed -- must be found and converted separately, by grepping for
    `python3 .claude/scripts/` specifically, not just for the 5 renamed filenames.

## Per-file migration

1. **`drive_loop.py`** (from `drive-loop.sh`): trivial. A `while True` loop calling
   `subprocess.run` on `drive_tick.py` (via `uv run`) at `DRIVE_INTERVAL`, with a
   periodic audit tick. Preserve: the exact interval/audit-cadence logic already in
   the bash version (read it fresh at implementation time, don't guess the numbers).

2. **`land_lane.py`** (from `land-lane.sh`): the highest-risk conversion. Preserve
   exactly:
   - The `flock`-on-`land.lock` serialization (`fcntl.flock` in Python has the same
     underlying semantics; the `-w 1800` bounded wait becomes a `signal.alarm`-based
     timeout or a manual retry-with-deadline loop around a non-blocking
     `LOCK_EX | LOCK_NB` attempt -- needs a real decision, flagged for review below).
   - `fire_tick()`'s detached-background invariant: bash's `(cmd &) 8>&-` pattern
     (spawn detached, explicitly close the inherited lock fd in the child) becomes
     `subprocess.Popen(..., start_new_session=True, close_fds=True)` -- `close_fds`
     defaults to `True` in Python 3 already (unlike bash, which inherits fds by
     default), which needs to be confirmed as an ACTUAL equivalent, not assumed
     during review, since the whole comment trail in `land-lane.sh` exists because
     fd inheritance bugs were hit for real multiple times.
   - `worker_free()`'s `pgrep`+`/proc/<pid>/cwd` scan: straightforward
     `subprocess.run(["pgrep", ...])` + `os.readlink(f"/proc/{pid}/cwd")`.
   - The generated-file conflict auto-resolution, the bounded 36x5s retry around a
     busy main checkout, the `git worktree`/`git branch` lifecycle -- all direct
     `subprocess.run` translations of the existing git commands, no behavior change.
   - `retrigger()`'s brief-writing and `simple_dispatch.py` re-invocation.
   - The new `land-patch` mode (git am onto a fresh branch, fall through to the same
     landing logic) -- refactor the shared logic into a real Python function
     (`land_one(row_id) -> bool`) exactly as it already is a shared bash function.

3. **`drive_tick.py`** (from `drive-tick.sh`): second-highest risk.
   - The tick-lock (`flock -n 9`), dev-server revival (`curl` health check +
     detached `pnpm dev`), `dispatch-state.py` calls (already Python -- becomes a
     direct function call or import instead of a subprocess round-trip, a real
     simplification opportunity worth taking since `dispatch-state.py` is already
     pure Python with no reason to shell out to itself).
   - The whole TRIGGER/STATE computation (inbox polling, round-buffer starvation
     check, delegated-row state, land-failed markers) -- direct translation of the
     already-simplified 2026-09-11 logic (verified working via the dry-run test
     during that change) into real Python control flow instead of bash string
     concatenation. This should get MORE readable in Python, not just equivalent --
     the existing bash's `TRIGGER="${TRIGGER:+$TRIGGER; }..."` accumulator pattern
     is exactly what real code (a list of trigger strings, joined once) does better.
   - The POLICY/CORE prompt text: unchanged STRUCTURE, but the CONTENT is a LIVE
     reference to script names, not documentation -- found by round-1 review, real
     and critical: the text says verbatim `run .claude/scripts/land-lane.sh
     land-patch ROW-ID PATCH-PATH` and lists "the four scripts (simple_dispatch.py,
     land-lane.sh, board-archive.py, round files)". This is read and acted on by the
     coordinator LLM every single tick -- missing this update means every tick after
     cutover instructs the autonomous coordinator to shell out to a file that no
     longer exists. Must be updated in the SAME commit as the rename, and re-verified
     by grepping the final POLICY/CORE strings for every old filename after editing,
     not just trusted from memory of what was changed.
   - Piping `claude -p --output-format stream-json` through `tick-stream.py`:
     `subprocess.Popen` with `stdout=PIPE` feeding into a call to `tick-stream.py`'s
     existing functions directly (in-process, not a second subprocess -- another
     real simplification, since `tick-stream.py` is already Python). BUT: the
     external `timeout --kill-after=30s 2700s` wrapper around the WHOLE
     `claude -p | tick-stream.py` pipeline is a real, load-bearing hard-kill
     guarantee (round-1 review: this is literally how the 90-minute lock-stall
     incident of 2026-08-31 got bounded) -- it does not go away just because
     tick-stream.py's own logic moves in-process. The `claude -p` subprocess call
     itself must keep an equivalent hard bound in the Python version.
     **`agent_loop.py`'s `run_bash`/`run_tool` pattern is NOT a drop-in template for
     this specific case** (round-2 review, correcting round 1's own recommendation):
     `run_bash` polls `proc.poll()`/a deadline WITHOUT touching `proc.stdout` at all,
     then reads the whole output ONCE after the process is already dead (its own
     comment explains this dodges a different bug: a backgrounded child holding the
     pipe open makes `communicate()` hang). `tick-stream.py`'s actual job is the
     opposite -- it must consume and render each JSON line WHILE `claude -p` is still
     running, which is the entire point of "keeps the loop terminal a live, readable
     trace" (drive-tick.sh's own comment). Reusing `run_bash`'s shape naively (poll
     without draining, read stdout only after killing) would either delay all output
     until the process ends/is killed, or risk `claude -p` blocking on a full pipe
     since nothing drains it during the poll-sleep. Decide explicitly between (a) a
     reader thread draining `proc.stdout` line-by-line concurrently with a separate
     deadline-timer thread that kills the process group on timeout, or (b) keeping the
     external `timeout` wrapper as a literal subprocess specifically BECAUSE it avoids
     this drain problem for free -- do not assume `run_bash`'s pattern transfers here.
     Separately, **`tick-stream.py`'s early-exit-on-terminal-event behavior must be
     preserved** (round-2 review, previously unflagged): it breaks out of its read
     loop the instant a `"result"` event arrives, deliberately NOT waiting for stdin
     EOF (its own comment: a leaked background-task fd can withhold EOF forever --
     this is a second, independent defense against the same 2026-08-31 hang class,
     not just the outer `timeout`). A naive `for line in proc.stdout: render(line)`
     merge into `drive_tick.py`'s own consuming loop would silently drop this early
     break, making every tick depend solely on the 2700s/30s outer bound to terminate
     promptly instead of exiting the moment the real answer is known -- a quiet
     regression of "fast when done" behavior. The in-process merge must keep an
     explicit break on the terminal event, not just iterate the stream to EOF.
   - The coordinator-auth-failure streak detection: direct translation, a small
     state file read/write.

4. **`opencode_worker.py`** (from `opencode-worker.sh`): direct translation --
   `subprocess.Popen` for the `opencode run` call, piping through `opencode-peek.py`
   in-process (same simplification as above), a `lanes.csv` telemetry append on exit,
   firing `land-lane.sh land` (now `land_lane.py`) on success.

5. **`tick_peek.py`** (from `tick-peek.sh`): trivial, a `tail -f`-equivalent piped
   through `tick-stream.py`'s existing render function, in-process.

## Cross-reference updates (every one confirmed by a real grep, not assumed complete)

Files that reference the old `.sh` names by path and need updating to the new `.py`
names and `uv run` invocation form:
- `.claude/skills/drive/SKILL.md`
- `.claude/skills/enwiro-delegate/SKILL.md`
- `.claude/skills/land-delegated-work/SKILL.md`
- `plans/simple-dispatch-design.md`, `plans/simple-dispatch-rollout-plan.md` (historical
  sections that name the old scripts -- update only where they describe CURRENT
  behavior, leave historical/dated entries describing what was true at the time
  untouched, matching this whole project's own provenance discipline)
- `.claude/scripts/tick-stream.py`, `.claude/scripts/dispatch-state.py` internal
  comments mentioning sibling script names
- **`drive-tick.sh`'s own `POLICY`/`CORE` prompt strings** (round-1 review finding,
  critical): these are natural-language instructions the coordinator LLM reads and
  acts on every tick, not comments -- see the `drive_tick.py` migration note above.
  After editing, grep the FINAL prompt strings for every retired filename as a
  real verification step, not a trusted-from-memory check.
- **`justfile`** (round-2 review finding, critical -- missed by round 1's own
  invocation-site inventory): `just drive`/`just tick`/`just peek` invoke
  `.claude/scripts/drive-loop.sh`/`drive-tick.sh`/`tick-peek.sh` by bare executable
  path (shebang + exec bit, no `python3`/`uv run` prefix) -- exactly the bare-path
  bug class open question 4 already describes, and this is the PRIMARY human entry
  point for starting the drive loop (the recipe's own comment: "Run ONE"). Must be
  rewritten to the `uv run --project ...` form in the same commit as the rename.
- **`.claude/checks/run.sh`** (round-2 review finding): invokes
  `python3 .claude/scripts/board-lint.py` on the same surface the Stop hook runs --
  a bare `python3`-prefixed call, not `uv run --project`, so it inherits the exact
  "silent version drift / ImportError on a host without global pyyaml" risk open
  question 4 warns about, just for a `python3`-prefixed site instead of a
  bare-shebang one. Convert this call site too, even though it isn't one of the
  5 renamed scripts.
- **`.claude/settings.json`**'s `SessionEnd` hook (round-2 review finding): calls
  `python3 "${CLAUDE_PROJECT_DIR:-.}/.claude/scripts/lane-telemetry.py"` on every
  session end, also bare `python3`, also needs conversion to `uv run --project` for
  the same reason.
- `plans/board-archive.yaml` (round-2 review finding): a tracked, 2164-line file with
  3 historical mentions of `drive-tick.sh`/`land-lane.sh`/`dispatch-worker.sh` inside
  archived board rows -- belongs in the same "historical record, leave as-is" bucket
  as `ONE_OFF_FIXES.md` below, just previously missing from either bucket's list.
- `.claude/tmp-brief-*.txt` (round-3 review finding): a whole tracked-file class the
  plan's "grep for retired filenames" methodology never enumerated as a class, only
  found ad hoc. Two exist today: `tmp-brief-land-lane-lock-sccache-inode-reuse-fix.txt`
  (documents an already-fixed, already-archived bug -- pure history, leave as-is) and
  `tmp-brief-module-routing-syntax-build.txt` (narrates a past `land-lane.sh` refusal
  for board row `module-routing-syntax-build`, which is STILL ACTIVE on
  `plans/board.yaml`, not yet archived -- so this one isn't purely dead history the
  way the others are, though its content is still safe to leave as a past-tense
  narration). Check this whole filename pattern during the cross-reference sweep, not
  just the two files known today -- more may exist by implementation time.
- `ONE_OFF_FIXES.md`, `plans/opencode-rollout.md`, `plans/prompt-efficiency-review.md`,
  `plans/brief-phrasing-experiment.md`, `plans/worker-pool.md` -- re-check each: most
  of these are dated incident logs (historical record), not live instructions: verify
  case by case whether a given mention is "what happened on this date" (leave as
  historical record with the old filename, since that's literally what ran that day)
  versus "what to do now" (update). Do not blanket-rename every historical mention --
  that would misrepresent what actually ran on a past date, the exact failure mode
  AGENTS.md's provenance section warns against for a different kind of record.

## Open questions, flagged for the skeptic rounds rather than pre-decided

1. **`flock -w N` (bounded wait) in Python**: `fcntl.flock` has no built-in timeout.
   Options: (a) a manual poll loop (`LOCK_EX | LOCK_NB` in a `while` with `time.sleep`
   and a deadline), (b) `signal.alarm` + `SIGALRM` handler interrupting a blocking
   `flock`, (c) a third-party lock library. Recommend (a) -- simplest, no signal
   interaction with subprocess handling elsewhere in the same script -- but this is
   exactly the kind of "looks fine, has a real subtlety" translation the skeptic
   rounds should pressure-test (a poll loop's sleep granularity changes the exact
   wait semantics; confirm it doesn't matter here).
2. **Detached background process fd hygiene** -- RESOLVED (round-1 review): bash's
   `(cmd &) 8>&-` pattern has a documented, real incident trail in this exact codebase
   (the fd-9/fd-8 leak bugs cited throughout `drive-tick.sh`/`land-lane.sh`'s comments).
   Confirmed via a real test (`/proc/self/fd` inspection in the child): `subprocess.
   Popen`'s default `close_fds=True` (Python 3.4+) really does close an `flock`'d fd
   in the child process, unlike naive bash backgrounding, which needs the explicit
   `9>&-`/`8>&-` dance. No special handling needed beyond just using `Popen` normally
   with `start_new_session=True` for detachment.
3. **Preserving `set -uo pipefail` discipline** -- RESOLVED/clarified (round-1 review):
   bash's `set -u` (undefined-variable crash) is exactly what caught 2 real bugs in the
   drive-tick.sh rewrite a few commits ago. Confirmed via matching real repros in both
   languages that Python's `NameError`/`UnboundLocalError` already reproduce the exact
   same runtime-only "only fails when the unset name is referenced" semantics as bash's
   `set -u` -- no special substitute is strictly required to match bash's guarantee.
   `ruff --select F821` does BETTER than bash, though: static (compile-time) undefined-
   name detection, not just a matching runtime crash. Decision: add `ruff` (with at
   least `F821` selected) as a dev dependency -- free extra safety beyond bash's own
   guarantee, cheap to add, no reason not to.
4. **How `uv run --project` gets invoked from EVERY call site** -- partly resolved,
   partly a NEW finding (round-1 review). Confirmed by real test: `uv run --project`
   works correctly from an arbitrary cwd, costs ~35-48ms warm invocation overhead, and
   showed no contention across 8 concurrent cold+warm invocations -- cheap enough to
   call directly at every site rather than needing a caching daemon or similar. BUT a
   more serious, previously-unnoticed problem surfaced while checking call sites for
   real: `land-lane.sh`'s `fire_tick()` (which calls `drive-tick.sh`), `drive-loop.sh`
   (which calls `drive-tick.sh audit`), and `opencode-worker.sh`'s `fire_next()` (which
   calls `land-lane.sh land ...`) all invoke sibling scripts by BARE EXECUTABLE PATH
   (e.g. `"$SCRIPTS/drive-tick.sh"`), relying on the shebang + exec bit, NOT via
   `python3 <path>` or any wrapper. A naive `.sh` -> `.py` rename that preserves this
   bare-path invocation shape would silently bypass `uv run --project` entirely and
   fall back to whatever global `python3` happens to be on `PATH` -- undermining this
   plan's own stated goal that `uv.lock` makes every invocation resolve identically.
   Dangerous specifically because it would NOT fail loudly on this machine (global
   `python3 -c "import yaml"` already succeeds here, confirmed by round-1's own test)
   -- it would only surface as silent version drift, or an `ImportError` on a host
   without global `pyyaml` installed. New explicit plan item: during implementation,
   inventory EVERY script-to-script invocation site (not just the ones already using
   `python3 <path>` today) and rewrite each to the `uv run --project <abs-project-dir>
   <abs-script-path>` form -- a single small shared shell/Python snippet or wrapper is
   fine, but every call site must be checked, not assumed fixed by the rename alone.
5. **Testing/verification strategy before cutover**: given `land_lane.py` and
   `drive_tick.py` drive REAL, LIVE autonomous git history changes, the implementation
   phase must include real verification, not just code review -- reusing the
   isolated bare-clone test harness already built earlier this session for
   `land-lane.sh land-patch`'s own validation, run again against the NEW `land_lane.py`
   with the exact same real patch, comparing outcomes. `drive_tick.py`'s trigger
   computation should get the same dry-run-against-the-real-repo treatment its bash
   predecessor already got.
6. **Rollout discipline**: the user's words ("no shell scripts and such") read as a
   full, immediate replacement, not a parallel-run trial window the way
   `simple_dispatch.py`'s own rollout used one -- but deleting `land-lane.sh`/
   `drive-tick.sh` before their Python replacements are REALLY verified working would
   repeat the exact "trusted without checking" mistake this whole session has
   otherwise been careful to avoid. Recommend: implement the `.py` versions, verify
   them for real (the isolated-clone test, a real dry-run), THEN delete the `.sh`
   originals in the same commit that lands the verified replacement -- no lingering
   both-still-present state, but also no unverified deletion.
