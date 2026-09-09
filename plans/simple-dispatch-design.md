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
- **Not yet tested**: the actual multi-turn tool-calling loop against a live
  model producing real edits, and the multi-row parallel fan-out under
  `ThreadPoolExecutor`. Both require actual OpenRouter credit, which the
  account does not currently have (per the same `/api/v1/credits` check).
  Re-run once credits are added: `python3 .claude/scripts/simple_dispatch.py
  <row> --brief-dir <dir> --parallel 3` with 3+ rows is the natural first
  real test of both at once.
