---
name: grill-via-annotations
description: Run a grilling round through the docs site's annotations mode instead of terminal dialogs - full rendered code examples, inline answers, quiet-period delivery. Use when a grilling session needs real code context (syntax decisions especially), or when the user asks to grill via the annotations page.
---

# Grilling through the annotations page

**RETIRED (2026-08-29): page-anchored standing questions.** Committed `@review`/`@comment`
HTML anchors on docs pages re-surfaced forever in the mail app as fresh-looking asks --
settled decisions got re-asked until the maintainer was, in his words, actively angry. A
question now lives in EXACTLY ONE place with state: a wizard/markdown round (ephemeral,
deleted the moment its answers are captured -- deletion is part of capture, verified, not
optional) or the decide queue on the board. Never commit a question into docs source.

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
- Option cards are SHORT (maintainer ruling, function-signature-matching-syntax round 4,
  2026-09-02: round 4's option blocks ran full code probes per option and drew explicit
  complaint -- "at least 8 times shorter"). One line of tradeoff plus a one-line verdict per
  option; no fenced code block inside `options[].description`. Put the shared code -- the
  baseline, the probes, the evidence that differentiates the options -- in `thesis` ONCE,
  referenced by each option rather than repeated per option. `background`/`thesis` still
  carry full rendered code; only `options[].description` shrank.
- Frame forward, not just for the immediate pick: name how each option would interact with
  known future plans/ambitions in this design area, not only whether it parses today. The
  maintainer does not want to commit early to an option that closes off a direction -- surface
  that tension in `thesis` rather than presenting a bare pick.
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

## Forest rounds (chain/tree, live) (kantord/toylang#grill-forest)

For a back-and-forth that can't be fully planned up front, or that genuinely branches on the
answer -- neither a markdown round's one discursive thread nor a wizard's fixed batch of separable
decisions fits that -- write a forest round: `docs/.grill/<topic>.forest.yaml` (gitignored,
ephemeral, a distinct extension from `.round.yaml` so the two never collide). Rendered live in the
tools app's own Grill section (`pnpm dev:tools`, a separate process from `pnpm dev` -- see
"Running the tools app" below), not the mail app.

Schema:

```yaml
topic: my-topic
activity:                  # optional list -- one entry per thread currently "in progress"
  - parent: <node id> | null  # null = a brand-new root/thread's first question being drafted
    note: Drafting a follow-up...
    since: <ISO timestamp>
nodes:
  - id: unique-slug         # required, unique within this file (not globally)
    parent: null            # null = a root (a new conversation thread); otherwise another node's id
    status: live             # draft | live | answered | superseded
    supersededNote: |         # REQUIRED when status: superseded -- why this question no longer stands
    title: Short label
    flow: question            # question | escalation | status, default question
    background: |             # optional markdown -- also where "why this follow-up" framing belongs
    thesis: |                 # optional markdown
    question: |               # required markdown, the direct ask
    options:                   # 0-4 proposed options; the human's UI always adds one more empty editor (cap 5)
      - label: Option name
        content: |              # markdown "mini user story" pre-filled into its editor
    answer:                     # present only once status: answered -- see "Answering" below
      sourceOption: Option name | null
      content: |
      wasEdited: true|false
      answeredAt: <ISO timestamp>
```

**Lifecycle**: append/edit nodes as `status: draft` freely -- the server never serves a draft node
to the browser, so this is where planning several questions ahead, and revising them as more
context arrives, actually lives. Flip a node to `status: live` to promote it: one plain edit, no
separate mechanism. If a live question stops applying before it's answered, set
`status: superseded` with a `supersededNote` (required -- a superseded node with none is rejected)
rather than just abandoning it: a stale ask quietly resurfacing once made the maintainer "actively
angry" (see the retired page-anchored design above), and a superseded node must say so.

**Superseding is precise about where the replacement goes -- get this wrong and the chain silently
stalls with no error, because there is nothing to validate against (the tools app just shows
"Superseded" forever and the coordinator has no way to notice from disk alone):**

- **The replacement is a SIBLING of the superseded node, not a child of it.** It must carry the
  exact same `parent` as the superseded node -- write it as another node under that same `parent`
  id, `status: live`, same as any other promoted node. A child of the superseded node is invisible:
  the chain only ever looks for a replacement among nodes sharing the superseded node's `parent`,
  never among its own children (a superseded node's own answer never comes, so it has no children
  to walk into). "Replace this question with a better one" reads naturally as authoring a child --
  it is not one here.
- **A supersede's own `activity` entry (if used) needs `parent` set to the superseded node's
  `parent`, not its id** -- the tools app looks for a thinking indicator under that same parent
  value, matching where the eventual replacement sibling will land.
- **Superseding the ROOT node of a thread is a dead end with no recovery in the same file.** A
  root has no parent for a replacement to share, so this is a deliberate, unhandled case, not an
  oversight (see the code comment on `activePath` in `grillForestChain.ts` if you want the full
  reasoning). If the very first question in a topic turns out wrong, don't supersede the root --
  either edit its still-`live` content directly if the human hasn't answered it yet, or abandon
  this topic file and start a fresh one (these files are ephemeral and disposable, same as any
  other round file in this directory).

**Progress feedback**: append an `activity` entry (`parent`: the node whose follow-up is being
drafted, or `null` for a brand-new thread's first question) the moment an answer is picked up,
before the real follow-up exists; remove it in the same edit that adds the real `live` node. The
tools app shows this as a "thinking" indicator. The same `parent`-not-id rule above applies when
the entry is for a supersede-and-replace rather than a fresh answer.

**Answering**: the human's answer posts to the same inbox door as everything else
(`POST /__annotations/save`, `page` = the forest file's path, `block` = the answered node's own
string id) the moment they submit -- process it immediately, same carve-out already given to
`.round.yaml` submissions, no quiet period. Processing means **writing the answer into the node
itself**, in this order:

1. Set the node's `status: answered` and fill in `answer` (`sourceOption`, `content`, `wasEdited`,
   `answeredAt`) -- write this to the forest file FIRST.
2. Only then clear the inbox record.

This order matters: the inbox is not durable storage here either (it gets cleared once consumed,
same as everywhere else in this system) -- the forest file is the only permanent record of the
answer. If a tick died between the two steps, the inbox record is still there next time, and
reprocessing it just rewrites the same answer, a harmless no-op; reversing the order would risk
losing the answer outright. Branching off a new answer: write the follow-up as a `draft` (or
several, one per branch under consideration), then promote exactly one to `live` once decided.

**Running the tools app**: `pnpm dev:tools` in `site/` is a separate Vite dev server/process from
`pnpm dev` (the docs site) -- `annotationsInbox()`, `grillRounds()`, and `grillForest()` all moved
there entirely, so the plain docs dev server no longer serves any of this. Start it the same way
as any other dev-server task the coordinator owns (background, verified free port) if it isn't
already running.

## When not to use it

A single quick ratification with no code context still goes through `AskUserQuestion` (with
previews). The annotations page (markdown round) earns its setup cost when a round carries one
discursive thread with real program listings; the wizard earns its when the round is really
several separable decisions that read better one at a time, each with its own options to weigh
side by side; a forest round earns its when the round branches on the answer or can't be planned
in full up front.
