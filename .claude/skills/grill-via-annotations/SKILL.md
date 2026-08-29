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

- Coordinator messages follow the maintainer's structure: sections labeled Background /
  Thesis / Question (only the ones needed, in that order), and the direct action or response
  needed from the human stated in ONE bold line -- first where possible, never buried. The
  site renders these as color-coded left-bordered sections per flow type; the authoring side
  supplies the labels. ADHD-communication rules apply: action first, scannable, bounded.
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
   that a round is up and where. A port that answers is NOT proof: a delegated worker's own
   dev server answers identically and dies with its worker mid-round (it lost round 1 of the
   auto-matchers grill at submit). The server must be one the coordinator started from the
   MAIN checkout, as a background task it owns -- if in doubt, check the listener's cwd or
   just start your own on a verified-free port before announcing the round.
2. Arm a poll: a cron tick every ~10 minutes reading `docs/.annotations/inbox.json`. The
   round is ready when the inbox's `last_edit` is at least five minutes old and covers the
   round's page. Do not process earlier -- partial answers are not answers.
3. On ready: read the per-block edits, map them to the questions, then clear the inbox.
   Ambiguous answers get a follow-up round, not a guess.
4. Capture decisions exactly as any grilling: draft.md sections, board rows, issues. A
   decision that lives only in `.grill` does not exist -- the directory is gitignored and
   disposable. Delete or overwrite round files freely once captured.

## Wizard rounds (kantord/toylang#34)

For a session with several questions that each want their own screen -- one decision, its
full context, its options with real code previews, next/back, a summary before submit -- write
a structured round instead of a markdown one: `docs/.grill/<topic>.round.yaml` (gitignored,
ephemeral, same directory as the markdown rounds above but a distinct `.round.yaml` extension
so the two never collide). YAML over JSON because the rest of the repo's structured data
(`plans/board.yaml`, the corpus) is already YAML, and block scalars (`|`) keep multi-line
markdown and code fences legible in the file itself.

Schema:

```yaml
intro: |                    # optional, markdown, shown on a "Begin" screen before Q1
  # Round title
  Framing prose.
questions:
  - id: unique-slug          # required, unique within the round; keys the wizard's answer
                              # state only. The inbox record's `block` is the question's
                              # array index -- `id` never reaches the inbox.
    title: Short label        # required, shown in the flow badge and the summary
    flow: question             # optional: question | escalation | status (default: question)
    background: |              # optional, markdown, full code blocks allowed
      ...
    thesis: |                  # optional, markdown
      ...
    question: |                # required, markdown -- the direct ask
      ...
    options:                   # optional
      - label: Option name
        description: One-line tradeoff.
        preview: |              # optional, real code as it would look under this option
          ...
        previewLang: toylang     # optional, defaults to toylang
    freeText: true               # optional: true, or a string used as the placeholder for the
                                  # free-text box. The box is always shown, options or not --
                                  # writing your own option is never gated on the round author
                                  # having added one (kantord/toylang#52).
```

The wizard renders each question's `background`/`thesis`/`question` as its own color-coded
left-bordered section (the design-system comment on the issue), one question per screen, with
a progress indicator, back/next, and a summary screen listing every answer before an explicit
Submit. A round is a type of mail (kantord/toylang#52): it arrives as an inbox item in the dev
server's mail app and is answered right there in the reading pane (`GrillWizard.tsx`, dev-only
and tree-shaken out of `vite build` the same way `MailApp.tsx` is), written by the coordinator,
read and answered by the maintainer -- no terminal round-trip in between.

**Delivery**: Submit posts one `/__annotations/save` record per question, `page` set to
`docs/.grill/<topic>.round.yaml` and `block` to the question's index, `edited` a small JSON blob
(`{"option": "...", "notes": "..."}`) rather than prose, per the issue's "shaped so the
coordinator can map answers to questions mechanically." **A record whose `page` ends in
`.round.yaml` is a wizard submission, not an incremental annotation edit: process it as soon as
it shows up, ignoring the quiet-period wait below.** The wizard already withheld the whole batch
until the maintainer pressed Submit; waiting five more minutes on top of that would be waiting
on nothing.

## When not to use it

A single quick ratification with no code context still goes through `AskUserQuestion` (with
previews). The annotations page (markdown round) earns its setup cost when a round carries one
discursive thread with real program listings; the wizard earns its when the round is really
several separable decisions that read better one at a time, each with its own options to weigh
side by side.
