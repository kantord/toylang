---
type: Reference
title: Code-style lessons
description: Index of the lessons the code-style checks point at. An Open Knowledge Format bundle, empty until the first escalations settle real cases.
tags: [code-style, index]
---

# Code-style lessons

How this repo has decided to handle each kind of finding the checks report. A
finding named `file-too-long` looks for `file-too-long.md` beside this index;
a lesson that grows too specific splits into `file-too-long/` and links
onward.

This is an [Open Knowledge Format](https://okf.md/) v0.2 bundle: markdown
with YAML frontmatter, cross-linked into a graph. `type` is the only required
key. Nothing here needs a runtime, an index to rebuild, or a tool to read it.

## Lessons

None yet, deliberately. A lesson records a decision this repo actually made
after a finding forced the question. toy-browser's eleven lessons were not
imported: they cite that repo's code as worked examples and record decisions
that repo made after escalations it had, and a lesson that exists before any
agent was blocked into writing it is less trustworthy than the rest (their
ADR-0008 records learning this the hard way). The first agent to trip each
kind escalates, and writes what gets settled. Where a toy-browser lesson
holds a repo-independent insight, cite it as a source when the lesson gets
written here.

An agent that meets a finding with no lesson is expected to stop and ask, not
to guess -- see [the playbook](/.claude/skills/code-style/SKILL.md). This
list grows as decisions get made, and each entry will link to the node that
records one.
