# Prompt for a privileged Claude Code agent -- issue-93 (euler-slow-fragments-2)

Maintainer ruling, captured 2026-09-01 (round `issue-93-euler-slow-fragments-2-escalation`):
after 4 commitless automated runs, hand this to a privileged interactive session instead of
redispatching. The lane is parked (`~/.cache/toylang-drive/escalated-issue-93` marker kept) so
the drive loop will not pick it back up. Paste the block below into a fresh Claude Code session
with normal (non-worker) permissions.

---

Fix gh:93 in kantord/toylang: apply the slow-fragment tier to Euler 23 and 27.

Two pages are currently stubbed as "(skipped)" prose with no code:
- `docs/examples/euler/23-non-abundant-sums.md`
- `docs/examples/euler/27-quadratic-primes.md`

Each page's own text already states the verified answer:
- 23 -> every number under 28123 that is not expressible as the sum of two abundant numbers
- 27 -> a=-61, b=971, product=-59231

There is no prior working toylang solution for either problem anywhere in this repo's history
(checked: no commit ever touched either file besides its initial "(skipped)" write, and no
orphan branch has it). You are authoring both solutions from scratch, not converting existing
code.

Constraints:
- No sorted Vec, no fast set membership (per gh:86) -- linear scan is fine, it is expected to
  be slow. That is *why* these are slow-fragment cases in the first place.
- Follow the pattern already landed for problem 14
  (`docs/examples/euler/14-longest-collatz-sequence.md`): a ` ```toylang slow ` fenced code
  block plus an `output` block, gated so `just test` only type-checks it and `just slow-test`
  actually executes it against all seven backends. Copy that file's structure exactly for the
  fence/gating mechanics -- only the algorithm content differs.
- Problem 23 takes about 42s on Go (the fastest backend) to actually run; problem 27 takes
  about 39s on jq. Budget for that when you run `just slow-test`.

Do this in two parts:

1. **Fix**: write and verify both solutions (author the toylang code, run it, confirm the
   outputs above), then wrap them in the slow-fragment pattern and land the change (`just test`
   and `just slow-test` both clean).

2. **Forensic diagnosis** (the maintainer wants this even though you're fixing it manually):
   four unattended automated runs on this exact task produced zero commits. Read the event
   logs for this lane in `~/.cache/toylang-drive/opencode/` (look for the issue-93 /
   euler-slow-fragments-2 runs) and figure out *why* the automated worker kept failing to
   write code. The last recorded run spent its steps reading builtins docs and probing the
   CLI (`cargo run -- run --help`) without writing any `.toy` file, then hit a denied direct
   binary execution (`target/debug/toylang run --help`) and stopped -- but that's only the
   last run; check all four. Report back (as a comment on gh:93, or a short note in
   `plans/opencode-rollout.md` if it's a rollout/tooling issue rather than a task-shape issue)
   whether this was: a brief-clarity problem, a genuine capability gap (worker couldn't author
   from-scratch algorithmic code under this constraint), a permissions/tooling trap (denied
   commands eating the run), or something else. That diagnosis is what tells us whether other
   "author from scratch" rows are going to hit the same wall.
