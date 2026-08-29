---
type: Playbook
title: An inherited file-too-long finding is not this session's to fix
description: What to do when a file was already over the line budget at merge-base and this session's own diff does not push it further over.
tags: [file-too-long, inherited-debt]
---

# An inherited file-too-long finding is not this session's to fix

`check.rs` was 1851 lines at this branch's original merge-base, already over
the 1000-line budget, before this session touched it. A run of style
cleanups (naming a couple of tuple returns as structs, extracting a few
helpers) left it at 1898 -- still over, and the finding still fires, but the
session neither caused the file being over budget nor grew it past what the
current merge-base already carries.

## What settled it

Nothing to do here. `plans/quality-practices.md`'s design for this check
says so directly: findings are meant to carry a caused/inherited split "so a
session is never asked to haul a 600-line refactor into a two-line change,"
and `clippy.toml` names `check.rs` as one of the "three right first
conversations" the 1000-line budget exists to *name*, not to force whoever
next opens the file to resolve. That conversation is a split, and
`plans/quality-practices.md` is explicit that the first such split (these
files are parallel in shape to several emitters) deserves a grilling
session, not an agent's improvisation mid-task.

React to this finding only when:

- **the session's own diff is what pushed the file over budget** (merge-base
  was under 1000, this branch is over) -- that is caused debt, and worth
  raising even if splitting the file is out of scope for the task at hand;
  or
- **the session was asked to address the file's size directly** -- then this
  lesson does not apply; that is the split conversation itself.

Otherwise, note in the session's own report that the finding is present and
inherited (the check's own message already says "already N lines at
merge-base"), and move on. See
[too-many-lines](/.claude/skills/code-style/lints/too-many-lines.md) for the
sibling case (a function, not a whole file) and the same reasoning applied
there.

The SoA-cursor-match session (issue #40) hit this on `emit_llvm.rs`: 1865
lines at merge-base, 1870 after a five-line comment-and-restructure fix
inside one function. Same rule -- the diff didn't cause the file being over
budget, it nudged an already-over file by a few lines while fixing an
unrelated bug, and that stays inherited.

The unused-variable session (issue #45) hit this on `check.rs`: 2291 lines
at merge-base, 2378 after adding the unused-parameter and unused-field
checks plus their tests. The task was implementing issue #45, not the
`check.rs` split (that is issue #51's own conversation), so this stays
inherited by the same rule.

Issue #51 is that split conversation, and it is the other bullet above:
the session was asked to address the file's size directly, so the
inherited-debt reasoning does not apply here -- this is the decision, not a
detour from one. `plans/checker-structure.md`'s survey recommended
extracting exactly two passes that never touch the fused `synth`/`expect`
engine (type resolution, and stream linearity plus dead-code pruning) into
`check/types.rs` and `check/linearity.rs`, 548 of `check.rs`'s 2380
lines, while explicitly declining to split the remaining fused core: it has
no per-area seam the way the backends do, and the survey names it as the
sinkhole mechanism's first exemption (`sinkhole-machinery`, issue #54,
`status: delegated`) instead of a further shrink target. The result,
`check/mod.rs`, is 1832 lines -- still over budget, on purpose, until that
exemption lands. The check reports it as a "new file" because the path
changed (`check.rs` to `check/mod.rs`); it is the same fused core, not new
debt. Nothing to do here until issue #54 lands and can record the formal
exemption.
