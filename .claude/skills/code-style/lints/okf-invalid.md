---
type: Playbook
title: okf-invalid only applies to the lints and research-log bundles
description: What to do when okf-invalid fires on a file that isn't actually part of the OKF bundle, and what a genuine occurrence in lints/ or research-log/ looks like.
tags: [okf-invalid, structure]
---

# okf-invalid only applies to the lints and research-log bundles

The landing session for #34 hit `okf-invalid` on
`.claude/skills/grill-via-annotations/SKILL.md`: "frontmatter has no type;
OKF requires it." That was a false positive, filed as #42 rather than
guessed at, since no lesson existed yet.

## What settled it

`.claude/checks/run.sh`'s check was meant to cover "lessons and
research-log notes" (its own header comment), but its case pattern was
`.claude/skills/*.md`. A `case` pattern's `*` matches `/`, so that glob
caught every `SKILL.md` in the repo, not just the lints bundle under
`.claude/skills/code-style/lints/`. Skill files carry Claude Code's own
frontmatter contract (`name`, `description`) -- a different schema from
OKF's (`type`, `title`, `description`, `tags`) -- and were never meant to
have a `type` key.

Fixed by narrowing the pattern to
`.claude/skills/code-style/lints/*.md | research-log/*.md`, matching what
the surrounding comment already said the check was for.

## What to do

- **Fires on a path under `lints/` or `research-log/`:** genuine. Add YAML
  frontmatter with at least `type:` (`Playbook` for "how to handle X",
  `Reference` for a note others link to) -- see this bundle's other lessons
  for the shape.
- **Fires on anything else:** the check's scope has regressed again.
  Escalate rather than adding a fake `type` to a file that isn't an OKF
  document.
