# simple_dispatch.py rollout plan

Written directly (no interactive grilling round -- explicitly skipped) after
the design + 3-round adversarial review cycles recorded in
`plans/simple-dispatch-design.md`. Covers everything identified as still
needing a decision or an action before `simple_dispatch.py`/`agent_loop.py`
can replace `sandbox_dispatch.py` in the live autonomous drive-tick loop.

## 1. What's actually still missing to go live

`drive-tick.sh`'s embedded policy currently does three things with the old
script that `simple_dispatch.py` does not yet do at all:

1. **Dispatch**: `nohup python3 .claude/scripts/sandbox_dispatch.py ROW-ID
   --brief PATH-TO-BRIEF &`, one row at a time, as WIP slots free up.
2. **WIP counting**: `python3 .claude/scripts/sandbox_dispatch_status.py
   --count` -- counts truly-live dispatch processes (not `msb list`, not
   `board.yaml status`, both confirmed stale in past incidents).
3. **Landing**: the old script calls `land-lane.sh` itself at the end of a
   successful run -- the coordinator never lands anything from
   `sandbox_dispatch.py`'s output directly today.

`simple_dispatch.py` as built only does (1)'s mechanics, in its own new CLI
shape, and does NOT do (2) or (3) at all -- it returns a `Result` with a
patch path and stops. That gap has to be closed one way or another before
this can run unsupervised.

## 2. Recommended integration shape

**Dispatch shape: batch, not one-row-per-nohup.** The original stated goal
for this whole rewrite was real parallelism ("massively in parallel"), and
`simple_dispatch.py` already has a working `ThreadPoolExecutor` for exactly
this. Recommend changing the coordinator's own dispatch step from "one
`nohup` per row, WIP tracked externally by counting processes" to: when N
ready rows exist (N up to the WIP cap), write all N briefs, then call
`simple_dispatch.py row1 row2 row3 --brief-dir <dir> --parallel 3` ONCE,
backgrounded with `nohup ... &`. This makes WIP tracking trivial (the
`ThreadPoolExecutor`'s own pool size is the cap -- there is nothing external
left to count or get stale) and is the actual point of building this.

This needs one small, cheap code addition first: `simple_dispatch.py`
currently requires a `--brief-dir` directory of `<row_id>.txt` files. The
coordinator's existing convention writes briefs to `plans/brief-<row-id>.md`
(a different name pattern, different extension). Cleanest fix: rename the
brief files to the `<row_id>.txt` convention at write time (a one-line
change in whatever writes the brief today), not adding a second CLI flag
just to avoid a naming mismatch -- one convention, not two.

**WIP counting**: gone entirely, per above -- no replacement script needed.
`sandbox_dispatch_status.py --gc` (orphan sandbox reclaim) can stay as-is;
it's unrelated to dispatch counting and still useful (`msb` sandboxes can
still be abandoned by a killed coordinator).

**Landing**: `simple_dispatch.py` deliberately doesn't call `land-lane.sh`
itself -- staying a pure dispatch primitive is consistent with "as simple as
possible" and keeps landing's own retry/conflict logic in one place instead
of two. Recommend the coordinator does it: after a batch call returns, for
every row reported `GREEN` in the summary, the coordinator runs
`land-lane.sh land <row>` itself (detached, exactly as it already does
today for the `land-lane.sh` re-run path in rule (3)(b) of the current
policy). This is a small policy-text change, not a code change.

**Non-GREEN outcomes (RED / FATAL / TIMEOUT / STUCK)**: `simple_dispatch.py`
has no escalation/mail flow by design. The autonomous loop still needs SOME
way to surface these to Daniel without him watching logs. Recommend the
coordinator composes a `docs/.grill/<row>-blocked.round.yaml` round itself
from the plain `Result` fields (`status`, `message`, `patch_path`) the first
time it sees a non-GREEN outcome for a row -- reusing the existing grill
-round mechanism (already built, unrelated to `sandbox_dispatch.py`), not
reviving `compose_escalation()`'s canned-text pattern. The four outcome
kinds map to different urgency:
  - `FATAL` -- almost always an account/credential problem (dead key, no
    credit). Don't compose a round; flag directly in the tick's own chat
    summary, same as the current API-key-expired handling.
  - `STUCK` -- the model hit the same failure twice; this is exactly the
    "needs a stronger model or human attention" case the old
    `compose_escalation` options existed for. One round, same options shape
    (stronger model / hand off / drop), built from the real (now correctly
    -normalized-comparable) verify tail.
  - `TIMEOUT` -- likely means the task's budget was undersized, not that
    it's unsolvable. Recommend a simple auto-retry ONE time with a larger
    `--overall-timeout` before escalating to a human round.
  - `RED` (ran out of retries, still failing, but not STUCK -- i.e. it *was*
    making distinguishable progress each attempt) -- same round shape as
    `STUCK` today, since this is the closest existing analogue to the old
    "attempts exhausted" escalation.

**`sandbox_dispatch.py` retirement**: don't delete yet. Keep it, unused, as
a fallback for a defined trial window (recommend: until 10 real rows have
gone through `simple_dispatch.py` cleanly, or 2 weeks, whichever comes
first). Delete `sandbox_dispatch.py`, `dispatch-worker.sh`,
`opencode-worker.sh`, `opencode-peek.py`, and the `opencode.jsonc`
generation logic together, in one commit, once that trial period is judged
successful -- not piecemeal.

**Policy text**: `drive-tick.sh`'s embedded `POLICY` string (a very long
literal in the script) references `sandbox_dispatch.py`,
`sandbox_dispatch_status.py --count`, and the per-row `nohup` pattern by
name. This needs a direct edit once the dispatch shape above is agreed --
not a "leave both scripts referenced and let the coordinator pick," since
that reintroduces exactly the kind of ambiguity that caused the coordinator
-collision bug this whole investigation started with.

## 3. Real-world validation, in order (nothing here has been done yet)

Only a single-row live smoke test has happened, against an exhausted
account, with `--parallel` never exercised for real. Before wiring into the
autonomous loop:

1. **Confirm real credit.** Check `/api/v1/credits` directly; don't proceed
   until there's meaningful headroom (a top-up may be needed -- the account
   was $0.17 negative as of the last check in this session).
2. **One real row, alone.** `simple_dispatch.py <a-real-ready-row>
   --brief-file <its-brief>` (after the brief-dir/naming fix above; a
   `--brief-file` single-path shortcut is also fine here if the batch
   -naming fix isn't done yet). Confirm: reaches GREEN or a correctly
   -classified non-GREEN status, produces a sane patch, and the sandbox
   tears down cleanly (`msb list` empty afterward).
3. **Three real rows, together, `--parallel 3`.** Confirm: all three
   sandboxes boot without host resource contention (watch `free -h` /
   `nproc` while it runs -- `--memory`/`--cpus` are configurable but nothing
   validates `--parallel * --memory` fits the host, so this first run IS
   the validation), the SUMMARY reports each row independently and
   correctly, and total wall-clock tracks the slowest row, not the sum of
   all three (this is the one thing the fake-dispatch concurrency test
   could NOT prove, since it used sleeps, not real sandboxes).
4. **Cost check.** Compare actual OpenRouter spend for these test rows
   against the historical per-row baseline from the original investigation.
   Nothing in this whole project has yet EMPIRICALLY confirmed the new
   pipeline is cheaper per row, only that specific waste mechanisms
   (collision, credit-blindness, unbounded thrashing) are closed -- that's a
   real, currently-unverified claim worth checking before calling the cost
   goal met.
5. **Land one for real.** Pick whichever of the above reached GREEN, run
   `land-lane.sh land <row>` on it by hand (not yet automated per section
   2), confirm it lands cleanly through the existing gate.
6. Only after 2-5 all succeed: make the `drive-tick.sh` policy-text edit
   from section 2 and let the coordinator drive it autonomously.

## 4. Deliberately deferred, not part of this rollout

- **Split-for-large-tasks.** No action until a real, genuinely cross-cutting
  multi-backend task actually thrashes under one growing session. Building
  it speculatively repeats the exact "never demonstrated to help" mistake
  found in the analysis-attack review.
- **Devil's-advocate-style sanity check.** Not re-added. Its old job
  (catching a plan-verdict/fact mismatch) has no equivalent surface in the
  new design -- there's no verdict to critique -- and the review found no
  other concrete gap it would close.
- **Automated host-capacity preflight for `--parallel`.** Manual check
  (section 3, step 3) first; automate only if this actually causes a
  problem in practice.
- **`normalize_for_stuck_check()`'s coupling to `cargo nextest`'s specific
  noise shape.** Documented as a known limitation, not fixed further: if
  `--verify-cmd` is ever pointed at a different tool, STUCK detection's
  noise-stripping won't apply to that tool's output and the exact-match
  blind spot could return for it specifically. Worth remembering if
  `--verify-cmd` ever changes, not worth generalizing pre-emptively today.
