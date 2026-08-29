---
type: Playbook
name: code-style
title: Reacting to a code-style finding
description: How to react when the code-style checks report a finding. Invoked by the Stop hook's message, not proactively; read it when a finding names it.
tags: [code-style, hooks, lessons]
---

# Reacting to a code-style finding

A check ran over the files this session touched and found something. Each
finding names a **kind**, and each kind has a **lesson**: a short file of
worked examples recording how this repo has decided to handle it.

```
file-too-long  src/emit_llvm.rs
  1666 lines excluding inline tests, budget is 1000 (inherited, already 1666 at merge-base)
  lesson: .claude/skills/code-style/lints/file-too-long.md
```

The lessons are an [Open Knowledge Format](/.claude/skills/code-style/lints/README.md)
bundle: plain markdown that cross-links into a graph. Start at the index
there. The bundle currently ships empty, on purpose; the index says why. While
it is empty, every finding leads here, and from here to *Escalate*.

## What to do

1. **Read the lesson.** If the file is missing, skip to *Escalate*.
2. **Follow its links.** A lesson is a small node, not a manual. It will point
   at more specific ones; the prose around a link says what it is for.
3. **Find the case that matches your situation.** Some cases carry a note
   saying they also fit other situations, or that a combination does.
4. **If one matches, follow it** and fix the finding.
5. **If none matches -- or what you found is too thin, too vague, or does not
   actually tell you what to do here -- escalate.** Do not improvise.

## Escalate

Escalating is the normal outcome for anything new. It is not a failure, and it
is not something to avoid by picking the nearest example and hoping.

**In a delegated session, escalate by filing a GitHub issue for the
coordinator -- never by waiting for a human.** The maintainer interacts only
with the drive loop's coordinator, not with delegated sessions, so an
`AskUserQuestion` in a delegated session parks it forever. Instead: file an
issue (`gh issue create`) stating what fired, why the lessons do not settle
it, and the real alternatives with what each costs -- "split by
responsibility, or by kind of item, or leave it and record the exemption?" is
an escalation; "how should I fix this?" is not. Then take the most
conservative continuation (leave the finding standing and the code as it is),
commit everything that IS settled, and end the turn. The coordinator triages
the issue: answers it where decided design already does, or turns it into a
decide row on `plans/board.yaml` for a grilling session.

**Only a session the maintainer is personally driving may `AskUserQuestion`
directly.** A subagent can do neither -- it has no channel at all; it reports
the escalation in its result and returns, and its spawner files the issue.

Do not fix the finding, and do not finish the escalated part as if the
question were not there.

## Writing what was settled

Write the outcome into the lessons bundle. Three rules, in order of
preference:

1. **Nothing new.** If an existing case already covers the situation, add one
   line to it saying so. Prefer this.
2. **A line, not a section.** If a combination of existing cases covers it,
   say which, in one sentence, where they are.
3. **A new node.** Only when the situation is genuinely new. Make it a *small
   file of its own* and link it from the parent lesson -- do not grow the
   parent. Where a toy-browser lesson holds the same repo-independent insight,
   cite it as a source, per AGENTS.md's citing rule.

Lessons are held to the same length budget as the code. That is deliberate:
it is what forces specific knowledge into linked nodes instead of one wall of
prose. Being as specific as you like is exactly what the graph is for.

## The file format

Every lesson is a valid OKF v0.2 document: YAML frontmatter with at least
`type`, then markdown.

```markdown
---
type: Playbook
title: Splitting an emitter that grew several jobs
description: What to do when one backend file accumulated unrelated responsibilities.
tags: [file-too-long, structure]
---

emit_llvm.rs held codegen, its runtime shims and the type mapping...
```

- `type` is the only required key. Use `Playbook` for "how to handle X",
  `Reference` for a note others link to.
- Link with plain markdown, bundle-relative:
  `[the split we did for check.rs](/.claude/skills/code-style/lints/file-too-long/by-responsibility.md)`.
  The link is untyped; the sentence around it carries the relationship.
- Unknown keys are preserved by any conformant consumer, so this file's `name`
  and `description` -- which the harness needs -- cost nothing.

## Suppressing a finding

Sometimes the right answer is that the check is wrong for this case. That is
also a decision worth recording: escalate, settle it, and write the exception
into the lesson so the next agent does not re-litigate it.

Never silence a finding by editing `limits.toml`, adding an `#[allow]`, or
splitting a file into meaningless pieces to get under a number. The budget
comes down over time by deliberate commits; it does not move to accommodate
one session.

Suppression is settled the same way as anything else, and almost never comes
back "yes". If you believe a check is measuring nothing here, look for the
better pattern first and bring *that* back -- an exemption is what is left
when the search failed, argued for in the open, never something a session
decides on its own to get unblocked.

A settled exemption moves into `.claude/checks/sinkhole.toml`: one entry per
finding it excuses, with a `justification` field, not a paragraph added to a
lesson. `.claude/checks/run.sh` consults it -- an entry suppresses the
finding it names, and a `file-too-long` or `too-many-lines` entry is
re-checked against the whole tree on every run, so one that stops firing
(the function shrank, the file split) is itself reported as
`stale-sinkhole-entry` instead of quietly outliving its reason; other kinds
have no automated re-check yet and stand on their justification. `#[allow]`/`#![allow]` beside the code it would excuse is itself
a finding (`bare-allow`); the sinkhole is the only sanctioned home. See the
file's own header comment for the entry format, and `check.rs`'s three
entries -- file length, `check()`, `synth()` -- as the worked example.

## The vocabulary

**Check** -- a program that reads the code and emits findings. Clippy is one;
the file-length check is another. Adding a check means adding a finding kind.

**Finding** -- one named complaint about one place in the code. The name is
the key to its lesson.

**Lesson** -- a node in the bundle under `lints/`. Missing, thin or unhelpful
all mean the same thing: escalate.

**Budget** -- a number in `.claude/checks/limits.toml` or `clippy.toml` that a
check compares against. Applies to prose and scripts as well as code. Lowered
deliberately, never to make a finding go away.
