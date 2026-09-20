---
type: Playbook
title: A collapsible-if finding swept in by file-touched scope
description: What to do when clippy's collapsible_if fires on code the session's own diff did not write, in a file the session touched for something else.
tags: [collapsible-if, inherited-debt]
---

# A collapsible-if finding swept in by file-touched scope

The recursion-improvements session (kantord/toylang#79) hit this on `emit_jq.rs`: the check
found `if *depth == 0 { if let Kind::Select { .. } = &base.kind { ... } }` inside `Kind::Index`'s
`expr()` arm, flagged only because the check reports every finding in a *file* the session
touched, not only lines the session's own diff added. Verified byte-identical at HEAD~2 --
pre-existing, only renumbered by this branch's own insertions earlier in the file -- the same
inherited/caused question [cognitive-complexity](/.claude/skills/code-style/lints/cognitive-complexity.md)
and [file-too-long](/.claude/skills/code-style/lints/file-too-long.md) already answer for their
own kinds. No lesson existed yet for this kind specifically, so the session escalated rather
than guess.

## What settled it

Fix it anyway, on the maintainer's explicit call (not the session's own judgment) -- this is a
purely mechanical rewrite with one meaning: `if a { if let b = c { body } }` becomes `if a &&
let b = c { body }`, using the `let`-chains clippy's own `--fix` suggestion proposes. Nothing
about it changes behavior or needs a design decision on its own; the *only* real decision was
whether an unrelated pre-existing finding is worth touching in an otherwise-scoped commit, which
is a maintainer call, not something to default either way on.

**The default absent a maintainer's call stays the cognitive-complexity/file-too-long rule**:
inherited, note it, move on, do not fold an unrelated fix into a task-driven commit. This entry
exists to record that a mechanical collapsible-if specifically was judged low-risk enough to be
worth a "yes, do it" when asked, not to establish that collapsible-if is always safe to fix on
sight -- ask again if the collapse is not this mechanical (e.g. the nested condition has its own
`else`, or collapsing would change which `?`/early-return fires).
