# Prompt experiment: generic reassurance phrase in dispatch briefs

Maintainer note (docs/.annotations/notes.json, composed 2026-08-31T18:22:26Z,
captured 2026-08-31 during drive tick): observed that a generic, content-free
reassurance -- variants of "it's much simpler than you think" -- said to a
planning agent tends to help it find a better plan. Directive: experiment on
at least 10 dispatched issues, track how well it works, then report progress
and hold a grilling session on the results.

## Protocol

- On each `dispatch-worker.sh` call for a build-kind board row, prepend one
  reassurance variant to the task-specific brief text (the `$2` argument),
  ahead of the standard boilerplate the script wraps around it.
- Rotate the wording tick to tick -- don't reuse the same sentence every time,
  so the log can separate "the technique" from "one lucky sentence."
- Add a row below per trial: issue #, phrase used, outcome, notes. Outcome is
  whichever of landed-clean / landed-with-rework / escalated / failed applies
  once the lane resolves.
- At 10 rows: stop adding new trials, write up the results, and open a
  docs/.grill/ round comparing against the un-reassured baseline (see the
  incident log in plans/opencode-rollout.md) asking whether to keep, drop, or
  refine the technique.

## Trials

| # | issue | phrase used | outcome | notes |
|---|-------|-------------|---------|-------|
| 1 | gh:151 (continuation) | "This is much simpler than it looks -- the prior run got 95% of the way there and only tripped on two duplicate-definition slips near the finish line." | pending | continuation dispatch, 2026-08-31 tick; fixes a duplicate `contains_sink` method + duplicate `ident()` match arm left by the prior run |
| 2 | gh:157 | "It's much simpler than it sounds -- one helper function is missing from one backend for one narrow program shape." | pending | fresh dispatch, 2026-08-31 tick |
| 3 | gh:155 | "Much simpler than a design change -- this is a pure mechanical removal, no new syntax to invent." | pending | fresh dispatch, 2026-08-31 tick |
