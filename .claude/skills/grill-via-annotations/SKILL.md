---
name: grill-via-annotations
description: Run a grilling round through the tools app's Grill tab (forest rounds in docs/.grill/*.forest.yaml) instead of terminal dialogs - full rendered code examples, inline answers, explicit submit. Use when a grilling session needs real code context (syntax decisions especially), or when the user asks to grill via the site.
---

# Grilling through the tools app

**RETIRED (2026-08-29): page-anchored standing questions.** Committed `@review`/`@comment`
HTML anchors on docs pages re-surfaced forever as fresh-looking asks -- settled decisions got
re-asked until the maintainer was, in his words, actively angry. Never commit a question into
docs source.

**RETIRED (2026-09-16): markdown rounds (`docs/.grill/*-round-<n>.md`) and wizard rounds
(`docs/.grill/*.round.yaml`), together with the Mail tab that rendered them.** The maintainer
ruled that grilling moves entirely onto the Grill tab's forest rounds; the five wizard
questions pending that day were migrated to one forest file each. A question now lives in
EXACTLY ONE place with state: a forest round (ephemeral, gitignored, its answer written into
the node the moment it is captured) or the decide queue on the board. `.round.yaml` files are
not served, not counted toward the round buffer, and not read by any tick -- do not write one.

The terminal collapses pre-question prose to a summary, so questions there cannot carry real
code. The Grill tab can: a node is a rendered markdown card with background, thesis, the direct
question, and option editors; the maintainer answers inline and submits explicitly.

## Writing a node

Content rules, inherited from every earlier mechanism and still binding:

- Follow the maintainer's structure: `background` / `thesis` / `question` (only the ones
  needed, in that order), and the direct action or response needed from the human stated in
  ONE bold line -- first where possible, never buried. ADHD-communication rules apply: action
  first, scannable, bounded.
- AGENTS.md prose rules apply. Every code fragment is real and was run -- the fence harness
  does not check `.grill` files, so honesty is manual here.
- Option cards are SHORT (maintainer ruling, function-signature-matching-syntax round 4,
  2026-09-02: option blocks that ran full code probes per option drew explicit complaint --
  "at least 8 times shorter"). One line of tradeoff plus a one-line verdict per option; no
  fenced code block inside `options[].content`. Put the shared code -- the baseline, the
  probes, the evidence that differentiates the options -- in `thesis` ONCE, referenced by each
  option rather than repeated per option. `background`/`thesis` carry the full rendered code.
- Frame forward, not just for the immediate pick: name how each option would interact with
  known future plans/ambitions in this design area, not only whether it parses today. The
  maintainer does not want to commit early to an option that closes off a direction -- surface
  that tension in `thesis` rather than presenting a bare pick.
- ONE live root per file: the server rejects two `live`/`answered` nodes under the same
  parent, and a root's parent is `null`, so a batch of separable decisions is several topic
  files, not one file with several roots. Name the topic after the decide row it serves.
- Delete-on-capture is gone: the answer is written INTO the node (`status: answered`), and the
  file stays as the thread's record until the row it serves is archived. A decision that lives
  only in `.grill` still does not exist -- capture it into draft.md sections, board rows,
  issues, exactly as any grilling.

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
string id) the moment they submit -- process it immediately, no quiet period. Processing means **writing the answer into the node
itself**, in this order:

1. Set the node's `status: answered` and fill in `answer` (`sourceOption`, `content`, `wasEdited`,
   `answeredAt`) -- write this to the forest file FIRST.
2. Only then clear the inbox record.
3. Act on it in the same tick -- board row, draft.md section, issue, or a child node that
   continues the thread -- and write `applied: <date> <what>` on the node. Durable is not
   applied: `seq-primitive-vs-existing-enum-machinery` was answered 2026-09-15 and written
   into the node, and no tick turned it into work for a day. `drive_tick.py` now flags every
   answered node that has neither an `applied:` line nor a child, on every tick, until it does.

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

## Choosing the mechanism

Two options, and the choice is easy to default away from under time pressure -- check
deliberately:

- **`AskUserQuestion`** (terminal, no code context): a single quick ratification, nothing to
  weigh side by side, no real code needed in the ask itself.
- **Forest round** (`.forest.yaml`, above): everything else. Separable decisions are separate
  topic files; a thread that branches on the answer, or can't be fully planned up front, is one
  topic file whose follow-ups chain as child nodes. The concrete tell that a follow-up belongs in
  the SAME file: the maintainer's answer asks for more exploration, raises a new sub-question
  inline, or is anything other than a clean pick from the options offered. A real miss,
  2026-09-12: `fold-and-infinite-streams`'s answer said, in part, "i think we have to explore
  this a bit, how other languages do it" -- an explicit request to keep going on the SAME
  thread. Composing that follow-up as a fresh cold topic throws away the actual "why" (a child
  node's `background` can quote exactly what prompted it).
