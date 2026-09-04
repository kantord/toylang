# One-off fixes for chronically stalled lanes

Written by the drive-tick automation on 2026-09-04, per maintainer instruction (composed
2026-08-03T20:47Z in the mail app): the automated worker loop has been paced across dozens of
retries, reshapes, and a model bump (GLM 5.2) on the items below and made zero net progress.
Rather than burn another retry cycle, these are handed here as paste-into-a-fresh-session prompts
for a privileged, non-worker Claude Code session. **The drive loop (`drive-loop.sh`) has been
stopped** so it won't redispatch these out from under you or fight over the same worktrees.

Each section is a self-contained prompt: paste the fenced block into a fresh interactive session
with normal permissions (not the sandboxed opencode worker). Work them in any order; they don't
depend on each other except where noted.

---

## Fix 1: stdin redesign (gh:172) — worst offender, 8 runs / 0 commits

```
Fix gh:172 in kantord/toylang: retire the three stdin keywords (`input`, `inputs`, `lines`) for
one `Stream<Str>` source, with `parse(.)` doing the type-directed parsing that `input`/`inputs`
used to do implicitly.

Worktree: ~/.local/share/toylang-lanes/issue-172 (branch issue-172, clean, sitting at commit
86c03af from 2026-09-02 -- eight automated dispatch runs since then produced zero further
commits). Read that commit's message and diff first; it's the closest thing to a running start
that exists.

Design, already ratified by the maintainer (don't re-litigate):
- `parse` grows a second checked overload: `Stream<Str> -> T` (join lines with \n, then parse as
  one value), alongside the existing `Str -> T`.
- `input` -> `parse(stdin)`
- `inputs` -> `stdin | map(parse(.))`
- `lines` -> plain `stdin`

Why automation couldn't do this in one shot (confirmed via plans/opencode-rollout.md and the
lane's own event logs): the full change spans src/ast.rs, src/parse.rs, src/tir.rs, both check
modules, all 8 emit_*.rs backends, ~30 corpus files, and ~27 Rust test files. Every automated run
-- including after a stronger-model (GLM 5.2) redispatch -- spent its whole step budget
re-deriving that scope from scratch and never reached a commit boundary.

Land it in independently-landable commits, in this order, committing after each one instead of
trying to do it all at once:
1. `parse()` gains the `Stream<Str> -> T` overload in src/parse.rs and src/tir.rs ONLY. Old
   `input`/`inputs`/`lines` keywords stay untouched and working. `just test` stays green.
2. Migrate ONE backend at a time (its emit_*.rs, its corpus subset, its Rust tests) to the new
   form -- one commit per backend, 8 backends total (check src/emit_*.rs for the full list).
3. Once every backend covers `parse(stdin)`, retire `input`/`inputs`/`lines` from src/ast.rs and
   the parser in a final commit.

Run `just test` clean after every commit. Push each commit as you go rather than batching -- if
you run out of time partway through, a partial migration in this order is still safe to leave on
main (old and new forms coexist until the final retirement commit).
```

---

## Fix 2: float formatting research + Go/Python/Rust/Lua backends (gh:149) — 6+ runs / 0 commits

```
Fix gh:149 in kantord/toylang: Float (IEEE binary64, ADR 0007) is done for JS (reference), C/LLVM,
and jq. It's still missing for Go, Python, Rust, and Lua, all four blocked on one prerequisite
that automation has failed to produce after 6+ dispatch runs (including a GLM 5.2 redispatch):
concrete per-backend float-formatting guidance.

Do the research first, write it to plans/float-format-research.md (it doesn't exist yet -- past
runs never got far enough to create it), then use it to implement the four backends.

Research needed: how each target formats an IEEE binary64 float to a string and parses it back,
matching JS's round-trip behavior (src/emit_js.rs is the reference -- read its Float handling
before starting):
- Go: strconv.FormatFloat / strconv.ParseFloat -- shortest round-trip mode is 'g'/-1 precision.
- Python: repr(float) / float(str) -- repr already gives shortest round-trip since Python 3.1.
- Rust: the {} (Display) vs {:?} (Debug) impls, and str::parse::<f64>().
- Lua (host hook backend): whatever Lua's tostring(number)/tonumber() actually do -- check this
  empirically, don't assume it matches the others.

For each, verify against real output (run it, don't just cite docs) that round-tripping a value
through format-then-parse is lossless, and check the same edge cases the C backend's
implementation had to handle (see runtime/toylang.c's Float formatter and its ~5000-value fuzz
verification approach, and check/mod.rs's Kind::Int(0)-for-Float-zero bug it fixed as a
cross-backend gotcha to check for in each of these four).

One more lead: ~/.local/share/toylang-lanes/issue-float-build-python has an uncommitted diff plus
scratch probe files (f64_probe, f64_probe.rs, float_check.py, hexdump.py, paren_probe.py) from a
real prior attempt -- read them (don't modify that worktree) for whatever was already
discovered about Python's edge cases.

Then implement, one backend per commit, following float-build's JS implementation as the pattern:
1. float-build-rust
2. float-build-python
3. float-build-go
4. float-build-lua (this one was explicitly parked waiting exactly on this research, per
   plans/board.yaml's float-build-lua entry -- do it after the research doc exists, not before)

Each backend gets its own commit, `just test` clean before moving to the next.
```

---

## Fix 3: Euler 23 & 27 slow fragments (gh:93) — already drafted, just needs running

This one already has a fully-drafted handoff prompt from a prior escalation (maintainer ruling,
2026-09-01, after 4 commitless automated runs). Nothing new to write -- just open and paste:

**`plans/issue-93-privileged-agent-prompt.md`**

---

## Separate item: docs dev server keeps dying (exit code 143)

Not a stalled board row, but flagged in the same maintainer note. Diagnosed, not yet fixed:

Every crash in `~/.cache/toylang-drive/devserver.log` / `dev-server.log` ends with
`[ELIFECYCLE] Command failed with exit code 143` -- 143 = 128+15 = SIGTERM. The drive-tick loop
starts vite with `setsid nohup pnpm dev ...`, which detaches it from the *terminal*, but `setsid`
alone does not detach it from the *systemd user session scope*. Confirmed on this machine:

```
$ loginctl show-user kantord | grep Linger
Linger=no
```

With lingering off, systemd-logind tears down the whole session's cgroup scope (SIGTERM to every
process in it, including the nohup'd vite) whenever the login session that originally launched
`drive-loop.sh` ends -- e.g. the terminal window it was started from gets closed. That matches the
143s exactly, and matches them clustering rather than being random vite bugs.

Fix (either one; second is more robust and needs no sudo):
```
loginctl enable-linger kantord
```
or launch the dev server via `systemd-run --user --scope ...` (or a proper `systemd --user`
unit) instead of `setsid nohup`, so it isn't a member of any login session's scope at all.
```

---

## Not included here: today's freshly-flagged stalls

The watchdog filed fresh stuck-lane investigations today (2026-09-04) for
`search-layer-build`, `recursive-descent-order-research`, `param-destructure-build`,
`http-query-sugar-research`, and `issue-177` (sort_by/max_by) -- all still at 1-2 runs, not yet
escalated. These haven't earned the "extremely stalling, tried everything" bar the items above
have; they're left for the normal drive-tick investigation flow once it's resumed, not bundled
into this one-off batch.

## Resuming automation

The drive loop is stopped, not removed. To restart it once the fixes above have landed (or
whenever you want automation back):
```
cd /home/kantord/repos/toylang && .claude/scripts/drive-loop.sh &
```
(run it detached -- a terminal or kitty window, per the script's own header comment.)
