Board row `stuck-issue-draft-records-migration-investigation`: investigation task, not a build
of the original feature. Do NOT attempt the original `draft-records-migration` task.

The `issue-draft-records-migration` lane stalled with no activity for over an hour, 1 run, 0
commits. Evidence is frozen in `plans/incidents/issue-draft-records-migration-20260907/` --
read `worktree-state.txt` there first, then look at the lane worktree itself
(`~/.local/share/toylang-lanes/issue-draft-records-migration`) for whatever state the worker
left behind.

Note: since that incident was captured, the lane has since been redispatched and did produce a
commit that is now going through landing (merge-conflict retries against `draft.md`) -- so
whatever caused the ORIGINAL stall (before any commit existed) may already be moot. Say so in
your report if that's what you find; don't force a diagnosis that no longer applies.

Investigate and report in `plans/opencode-rollout.md` (append, matching the existing incident
log style) whether the original stall was:
1. brief clarity (the task as written was ambiguous or under-specified),
2. a capability gap (the worker/model couldn't do the task at all),
3. a tooling/permission trap (something blocked without a clear failure signal), or
4. task shape (the task should have been decomposed differently).

Propose a concrete rebrief or reshape recommendation either way, even if you conclude the
original stall is now moot because the lane already recovered on redispatch.

Done-gate: `plans/opencode-rollout.md` has a new entry covering this investigation with a clear
diagnosis and recommendation, `just check` still passes (no leftover repro code).
