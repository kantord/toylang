---
name: grill-via-annotations
description: Run a grilling round through the docs site's annotations mode instead of terminal dialogs - full rendered code examples, inline answers, quiet-period delivery. Use when a grilling session needs real code context (syntax decisions especially), or when the user asks to grill via the annotations page.
---

# Grilling through the annotations page

**The standing convention comes first: the annotations sidebar is the maintainer's inbox.**
Any open question the coordinator has about real content lives as an annotation ON the page
it concerns (an HTML comment, invisible to readers, surfaced by the sidebar) -- not in chat,
not only on a round page. Round pages in `docs/.grill/` hold what has no doc anchor: repo-
internal decisions, multi-question sessions, free boxes. The maintainer opens annotations
mode, sees everything pending, answers in place; the inbox delivers.

The terminal collapses pre-question prose to a summary, so questions there cannot carry real
code. The annotations mode can: a round is a rendered markdown page, the user answers inline,
and the inbox delivers the whole round after five quiet minutes.

## Writing a round

Write `docs/.grill/<topic>-round-<n>.md` (gitignored, ephemeral). It renders in the site's
annotations mode and its annotations join the sidebar; it never appears in public nav. Rules:

- AGENTS.md prose rules apply. Every code fragment is real and was run -- the fence harness
  does not check `.grill` files, so honesty is manual here.
- Each question is one annotated span: `<!-- @fill ... -->` where an answer gets typed,
  `<!-- @review ... -->` where a recommendation needs confirming or vetoing,
  `<!-- @comment ... -->` for coordinator commentary that frames a question.
- Present options as full rendered blocks: the code as it would look under each option,
  costs stated, recommendation marked. This is the entire point of the mode -- never
  compress options back into one-liners.
- End the page with a short "what happens on submit" note so the user knows what their
  answers trigger.

## Running it

1. Start the dev server if not running (`pnpm dev` in `site/`), tell the user in ONE line
   that a round is up and where.
2. Arm a poll: a cron tick every ~10 minutes reading `docs/.annotations/inbox.json`. The
   round is ready when the inbox's `last_edit` is at least five minutes old and covers the
   round's page. Do not process earlier -- partial answers are not answers.
3. On ready: read the per-block edits, map them to the questions, then clear the inbox.
   Ambiguous answers get a follow-up round, not a guess.
4. Capture decisions exactly as any grilling: draft.md sections, board rows, issues. A
   decision that lives only in `.grill` does not exist -- the directory is gitignored and
   disposable. Delete or overwrite round files freely once captured.

## When not to use it

A single quick ratification with no code context still goes through `AskUserQuestion` (with
previews). The annotations page earns its setup cost when a round carries several questions
or real program listings.
