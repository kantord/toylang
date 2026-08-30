# The opencode delegation rollout log

Maintainer ruling, 2026-08-30: claude-code-based delegation is **retired**. All new
delegated build work dispatches through opencode + DeepSeek V4 Flash
(`.claude/scripts/opencode-worker.sh`; the enwiro-delegate skill carries the flow).
No new delegated work happens with claude code until the re-evaluation -- in-flight
claude lanes at ruling time finish and land normally.

This file is the coordinator's OBSERVABILITY OBLIGATION for the rollout: the flip is
provisional, and the evidence for keeping or reverting it accumulates here, not in
anyone's memory.

## Re-evaluation gate

After roughly **30 landed opencode lanes** (count: lanes.csv rows with kind=worker and
a deepseek model, cross-checked against the archive), the coordinator boards a `decide`
row -- `opencode-rollout-review` -- attaching this log and the cost/quality comparison
against the pre-rollout claude baseline in `~/.cache/toylang-drive/lanes.csv`. Until
that ruling, the default stays opencode.

## Incident log (append-only; the coordinator records EVERY issue, small or large)

Record at minimum: date, lane/issue, what went wrong, what it cost (retries, review
findings, coordinator interventions, abandoned work), and whether a claude worker
would plausibly have avoided it. Landings with zero incident need no entry -- absence
of entries over many lanes is itself the finding.

| date | lane | what happened | cost | claude-proof? |
|------|------|---------------|------|---------------|
| 2026-08-30 | issue-88 (csv-inputs-idea, gh:88) | Worker correctly diagnosed the task as a design decision (DSV delimiter vs. nullary sources) rather than a build, but its own bash call to file the follow-up issue got auto-rejected by the permission gate, so it exited after 14 steps with zero commits and no issue filed -- a dead lane with no visible trace besides its event log. Coordinator posted the analysis to gh:88 and reclassified the board row to `decide` by hand. | $0.01, 14 steps, one coordinator intervention (comment + reclassify) | No -- the permission auto-reject would block a claude worker's `gh issue create` too; not opencode-specific. The board row was simply mis-scoped as `build` when it should have started as `decide`. |

## Speedups shipped with the rollout (2026-08-30, same day)

- **Event-driven landing**: the worker wrapper fires a drive tick on exit; the tick
  gate lands worker-gone + ahead + clean immediately (no 8-minute quiet window --
  that heuristic existed because a claude turn-end was indistinguishable from a
  crash; an opencode exit is unambiguous).
- **Enwiro-free lanes**: `dispatch-worker.sh` = worktree under
  `~/.local/share/toylang-lanes` + background worker; the gh:124 worker pool and
  per-issue enwiro envs are legacy, kept only until their in-flight lanes land.
- **sccache** as RUSTC_WRAPPER in the worker env: cold worktrees share compiled
  crates; landed lane worktrees are removed at landing, so disk stays flat.

## Baseline, for the eventual comparison

- Trial lane (gh:114, 2026-08-30, pre-rollout): mid-tier emit_llvm refactor, landed
  end to end, $0.04 total, zero review findings, one self-corrected compile error.
- Known limitations going in: no server-side classifier (mitigated by the deny-by-default
  allow-list config in the maintainer's chezmoi, never `--auto`); no container/egress
  isolation yet (published best practice for unattended runs; accepted for this
  self-authored public repo); worker cannot receive SendMessage nudges (steer by
  killing + `opencode run --session <id>` resume with a new message).
