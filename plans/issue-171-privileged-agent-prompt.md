# Prompt for a privileged Claude Code agent -- issue-171 (http-query-sugar-build)

Maintainer ruling, captured 2026-09-08 (round `http-query-sugar-build-sandbox-blocker`,
question `http-query-sugar-build-sandbox-blocker`): option B -- after 3 verified sandboxed
build attempts against the real toolchain (full permissions, real LLVM/cargo-nextest, real
`just check` each round) reached the retry cap without going green, hand this to a
privileged/human session rather than more automated attempts. The lane is parked
(`~/.cache/toylang-drive/escalated-issue-171` marker kept) so the drive loop will not pick it
back up. Paste the block below into a fresh Claude Code session with normal (non-worker)
permissions.

---

Fix gh:171 in kantord/toylang: `http-query-sugar-build`.

Build HTTP query sugar per `http-query-sugar-design`'s ruling (2026-09-08): `fetch(url)` on
the 3 TLS-capable backends only (Go, JS, Python) -- full response record (status/headers/body)
via proper structs/enums, not an ad hoc record; a new header representation is needed since
toylang has no Map type yet. Lua/jq/Rust/native refuse this program class outright (they have
no TLS-capable HTTP client available -- make them error clearly at compile time for any use of
`fetch`, matching how other backend-restricted builtins are refused elsewhere in the compiler).
Once this lands, file a follow-up row for extending HTTP support to the remaining 4 backends
(maintainer flagged the design is not "really final" until all backends have it, but explicitly
scoped that out of this build).

The sandboxed dispatch got through a plan-decompose phase (with a devil's-advocate review of
the verdict) and 3 full build+verify rounds against the real toolchain, but every round's last
verify turn reported:

```
This turn made ZERO file changes (git status --porcelain was completely empty and HEAD never
moved from the starting commit). A passing `just check` here is trivially true, not evidence of
progress. Actually IMPLEMENT the task now: edit the real target files described in the brief.
Do not just explore, read code, or write throwaway test/repro scripts.
```

No patch was ever extracted (the worker made no edits across all 3 attempts, despite explicit
feedback each round), so there is no partial diff to resume from -- you are starting from a
clean `main`, not continuing anyone else's work.

Do this in two parts:

1. **Fix**: implement `fetch(url)` per the ruling above, verify with `just check` (full
   398+-test suite across all 7 backends), and land the change.

2. **Forensic diagnosis** (the maintainer wants this even though you're fixing it manually):
   3 unattended automated build turns on this exact task produced zero file changes each time,
   despite the model (`openrouter/deepseek/deepseek-v4-flash-0731`) actually running and
   `opencode`/toolchain invocations succeeding (this was not an API-key or infra failure -- see
   `~/.cache/toylang-drive/sandbox-dispatch-http-query-sugar-build.log` for the full transcript).
   Figure out why the worker kept exploring/reading without ever writing a `fetch` implementation.
   Report back (as a comment on gh:171, or a note in `plans/opencode-rollout.md` if it's a
   rollout/tooling issue rather than a task-shape issue) whether this was a brief-clarity
   problem, a genuine capability gap for this model tier on a multi-backend builtin this size, or
   something else -- other TLS/multi-backend-builtin rows may hit the same wall.
