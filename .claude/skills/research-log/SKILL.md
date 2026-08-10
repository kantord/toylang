---
name: research-log
description: Capture what building toylang taught us as an OKF note in research-log/. Use after finding a bug worth remembering, hitting a dead end, discovering a target-language constraint, or learning something that changes how the next step gets built. Also use when asked to write up a finding, or to check research-log/ for orphans and duplicates.
---

# research-log

`research-log/` records **findings**. `draft.md` records the design and `plans/` records the
build order; both may cite a note here instead of restating it. If what you are about to write
is a decision rather than something learned, it belongs in `draft.md` or a commit message.

## Before writing, check it is not a duplicate

Read `research-log/index.md` first. Every note is listed there with its description, which is
enough to tell whether the idea already exists.

If an existing note covers the idea, **extend that note** rather than adding a second one. Two
notes that are each half an idea are worse than one note, because neither will be found. Merging
is also the right move when a new finding turns out to be another instance of an existing one:
add the instance to the existing note and strengthen its claim.

## Writing the note

One idea per file. If the note needs the word "also" to introduce its second half, it is two
notes -- or the second half belongs somewhere else.

Filename is the claim in kebab-case, as a sentence a reader can agree or disagree with:
`a-second-type-is-what-makes-a-checker-falsifiable.md`, not `type-checking-notes.md`. A filename
that could head a chapter is a topic, not a note.

Length: enough to state the finding, the evidence for it, and what follows from it. Roughly a
screen. If it runs past two, either it is several notes or most of it is restating the evidence.

Frontmatter follows the OKF schema:

```yaml
---
type: Lesson            # or Note, or invent one when nothing fits
calendar:
  - 2026-08-10          # when the thing was learned
title: A sentence stating the claim
description: One sentence, 100-250 characters. Reused verbatim as this note's line in index.md, so write it to be read there.
tags:
  - testing
timestamp: 2026-08-10T00:00:00Z
---
```

Write what was observed, with the concrete case that produced it. "The forward call resolved to
a global and died at runtime" is the note; "be careful with scoping" is not. A finding with no
evidence attached is an opinion, and it will not survive being disagreed with in three months.

Say what is still open. A note that only contains what is settled is a note nobody needs to
revisit, and the open questions are what make it worth rereading.

## Linking, and no orphans

Every note must be reachable. Two obligations, both required:

1. Add its line to `research-log/index.md`, using the `description` verbatim.
2. Link it from at least one sibling note, and link at least one sibling from it. If nothing
   connects, ask why the finding belongs in the same body of work at all -- usually something
   does connect and the link is the most valuable part of the note.

Links are relative markdown within `research-log/`: `[claim](other-note.md)`.

Prefer linking to a note over restating it. The moment a paragraph reproduces another note's
argument, cut it down to a link.

## Check before committing

```sh
for f in research-log/*.md; do
  n=$(basename "$f")
  [ "$n" = index.md ] && continue
  grep -q -- "($n)" research-log/index.md || echo "ORPHAN: not linked from index: $n"
  links=$(grep -l -- "$n" research-log/*.md | grep -v -e "research-log/index.md" -e "^$f$" | wc -l)
  [ "$links" -gt 0 ] || echo "ORPHAN: no sibling links to $n"
done
```

Silence means both obligations hold. Confirm the check still reports something by deleting a
line from `index.md` and rerunning: a check nobody has seen fail is not evidence of anything,
which is the whole of
[a test that cannot fail is worse than no test](../../../research-log/a-test-that-cannot-fail-is-worse-than-no-test.md).

Then the repo's own rules apply: plain ASCII throughout, and the commit message carries the
provenance lines from `AGENTS.md`.
