---
status: proposed
---

# Coordinator memory: a capped fact pool for `drive_tick.py`

## Problem

Ticks re-derive the same static facts and footprint conflicts repeatedly, because
`drive_tick.py` has no place to record a fact once and read it back later. The board
(`plans/board.yaml`) tracks live work; `plans/simple-dispatch-design.md` tracks incident
and design history. Neither is a lookup a tick can cheaply consult mid-turn for "have I
already established this." Nothing today fills that gap, and nothing should be invented
to fill it beyond what a tick's real work actually produces.

This proposal adds a small, capped memory pool -- `plans/coordinator-memory.yaml` -- that a
tick writes to as a byproduct of real work (never as a dedicated lookup task), and reads
from as part of its existing state snapshot. It ships empty and earns its keep or gets
retired the same way `sandbox_dispatch.py` and the opencode workers were.

## Design

### Scope: what belongs in the pool

Two kinds of fact, and nothing else:

- `footprint-conflict` -- two in-flight or planned pieces of work touch the same file/area
  in a way that isn't yet visible from the board alone.
- `fact-check` -- a static fact about current code (a line number, a function's existence,
  a policy string's exact wording) that a tick verified once and would otherwise
  re-verify on a later tick.
- `other` -- anything that doesn't fit the first two but is still a byproduct of real work,
  not a diagnosis narrative or incident writeup.

This is enforced mechanically, not by convention: a slot's `kind` is validated against a
`MEMORY_KINDS` enum in the lint script, so a tick cannot write `kind: stall-diagnosis` or
any other incident/procedure narration into the pool. Incident and procedure history keeps
its existing, better home: `plans/simple-dispatch-design.md`, which `TICK_POLICY` already
names as the canonical place for "a real, surprising incident" (`.claude/scripts/drive_tick.py:385-388`,
verbatim, and unaffected by this change).

### Schema

`plans/coordinator-memory.yaml`:

```yaml
version: 1
cap: 8
slots: []
```

Ships with `slots: []` -- no seed data (see "Why the pool launches empty" below). Once a
tick writes to it, a slot looks like:

```yaml
- id: <short-slug>
  kind: footprint-conflict | fact-check | other
  summary: <one line -- the fact a tick would otherwise re-derive>
  source: <file:line, or the artifact the fact anchors to>
  written: <tick/date>
  confirmed: <count of times a later tick relied on it and it still held>
```

One invalidation rule: if `source` no longer resolves to the claimed fact -- the cited
file:line has moved or the conflict it described has resolved -- the slot is dropped rather
than left to go stale. This is what the audit bullet below checks for.

### Read path

New functions `_memory_slot_state()` and `_memory_signal()` (analogous in shape to the
existing `_*_signal` functions such as `_land_failed_signal()`) render each live slot as a
compact `[mem:...]` line and fold it into the tick's existing state snapshot, the same way
every other signal does:

```python
state_parts += safe_signal("coordinator memory", _memory_signal, default=[])
```

wired into `compute_trigger_and_state()` (`.claude/scripts/drive_tick.py:251-306`) alongside
the existing `state_parts +=` lines (e.g. 259, 263), and going through `safe_signal`
(`drive_tick.py:132`) for the same per-signal failure isolation every other state source
already gets -- one broken slot degrades to an empty signal, it doesn't take the tick down,
matching the isolation `_process_delegated_row`'s docstring already documents at
`drive_tick.py:151-162`.

Cost: at `cap: 8` and roughly 62 tokens per rendered `[mem:...]` line, worst case is ~500
tokens added to the state snapshot -- flagged as an open question below, since nobody has
confirmed the snapshot has that headroom to spare.

### Write path

A tick writes a slot only as a byproduct of real work -- noticing a footprint conflict or a
fact worth not re-deriving while doing something else, never as a dedicated "check memory"
step. This is stated directly in `TICK_POLICY` (`drive_tick.py:322-407`), as a new
paragraph placed immediately before the existing incident-note sentence (~line 385), so the
two adjacent rules read as one deliberate fork: incident narrative goes to
`plans/simple-dispatch-design.md`, footprint/fact-check notes go to
`plans/coordinator-memory.yaml`.

### Audit

One paragraph appended to `AUDIT_POLICY` (`drive_tick.py:309-320`): for every live slot,
re-resolve its `source` and confirm the fact still holds; bump `confirmed` when it does,
drop the slot per the invalidation rule when it doesn't. This costs zero new code paths
when the pool is empty (launch state) and is what makes the pool an active, checked cache
rather than a second source of truth nobody rereads.

### Lint

New function `lint_memory()` validates `plans/coordinator-memory.yaml`'s shape --
`version`, `cap`, each slot's `kind` against `MEMORY_KINDS`, slot count against `cap` --
and is wired into `board-lint.py`'s `main()` as a third call, alongside the two that already
run there (`board-lint.py:57-58`).

## Why the pool launches empty

The task brief's original guess was to seed the pool from
`.claude/skills/drive/SKILL.md` ("Stall diagnosis, learned the hard way"). Checked
against both the pool's own scope rule and the actual content, that doesn't hold:

- **Wrong kind.** That section is stall-diagnosis narrative for the retired
  `sandbox_dispatch.py`/opencode pipeline (dead-worker signatures, `ESCALATION.md`,
  opencode event logs) -- exactly the incident/procedure category the pool's `kind` enum is
  built to exclude. `lint_memory()` would reject an honest attempt to file it.
- **Already preserved, better, elsewhere.** The same history is already recorded in more
  useful depth in `plans/simple-dispatch-design.md`, the file `TICK_POLICY` already names
  as the canonical home for exactly this kind of note. Seeding the pool from SKILL.md would
  duplicate content that's already live there.

So that section isn't raw material for this proposal -- it's dead weight regardless of
whether the rest of it ships: the section already says out loud that it's superseded and
kept only as history, and it costs every tick real tokens to carry. This proposal deletes
it as its own independently-justified cleanup (see rollout step 1).

The consequence: **no backfill step.** The pool fills the first time a tick, doing its real
work, actually trips over a footprint conflict or a fact worth not re-deriving -- the same
way `simple_dispatch.py`'s replacement proved itself, and consistent with the "byproduct of
real work, never a dedicated lookup" rule the write path already states. Force-seeding it
from unrelated content would be building inventory nobody asked for.

## Files touched

1. **`.claude/skills/drive/SKILL.md`** -- delete the "Stall diagnosis, learned the hard
   way" section (the superseded/historical paragraphs). Nothing replaces the content; the
   section header either goes too, or becomes a one-line pointer: "Stall diagnosis: read
   `plans/simple-dispatch-design.md`; nothing pipeline-specific belongs here." (Maintainer's
   call -- see open questions.)
2. **`plans/coordinator-memory.yaml`** (new) -- `version: 1`, `cap: 8`, `slots: []`. Ships
   empty.
3. **`.claude/scripts/drive_tick.py`** -- add `_memory_slot_state()` and `_memory_signal()`
   near the other `_*_signal` functions; wire `state_parts += safe_signal("coordinator
   memory", _memory_signal, default=[])` into `compute_trigger_and_state()` (251-306);
   append the audit paragraph to `AUDIT_POLICY` (309-320) and the write-path paragraph to
   `TICK_POLICY` (322-407), immediately before the existing incident-note sentence (~385).
4. **`.claude/scripts/board-lint.py`** -- add `lint_memory()`; wire
   `errs += lint_memory("plans/coordinator-memory.yaml")` into `main()` as a third call,
   alongside the existing two (57-58).

No new script, no new commit discipline, no new file format: `plans/coordinator-memory.yaml`
is a sibling of `plans/board.yaml` and `plans/board-archive.yaml` in shape and in how it
gets edited -- by the tick, in the same commit as its real work.

## Rollout

1. Delete the dead SKILL.md section as its own small commit. Independently justified
   regardless of whether the rest of this proposal lands.
2. Add the empty `plans/coordinator-memory.yaml`, the `drive_tick.py` read/write wiring,
   and the `board-lint.py` lint, as one commit -- none of it does anything until a tick
   actually writes a slot.
3. No forced first slot. The first real footprint conflict or static fact a tick trips
   over gets written as slot 1, in that tick's own commit, per the new `TICK_POLICY`
   paragraph.
4. **Kill criterion**, carried over unchanged from every prior round of this design: same
   standing as `sandbox_dispatch.py`, `dispatch-worker.sh`, and the opencode workers, all
   retired for not earning their keep. Judge it at the next periodic audit or two after
   slot 1 lands. The bar is "did this measurably stop a re-investigation" -- checked via the
   audit's confirm-count (does anything ever get relied on and bumped) -- not "is the file
   tidy."

## Open questions

- `cap: 8` has no measurement behind it, just "a small number." Fine as a starting guess --
  should it start smaller (4-5) until the mechanism proves it fills with anything useful,
  or is 8 fine to lock into the lint script's `MEMORY_CAP` constant now?
- Where should the new `TICK_POLICY` paragraph actually land: folded next to the existing
  incident-note sentence (this proposal's placement, chosen for readability) or as its own
  numbered step in the policy's ORDER list?
- Delete the "Stall diagnosis, learned the hard way" section header in SKILL.md entirely,
  or leave a one-line pointer to `plans/simple-dispatch-design.md` in its place? Cosmetic,
  either is fine.
- Does the state snapshot the gate script builds have confirmed headroom for the pool's
  worst-case ~500 tokens (8 slots x ~62 tokens each), or does that need checking against
  current snapshot size before the cap is finalized?

## Panel provenance

Produced by a 5-round Skeptic/Creative/Pragmatist panel (2026-09-11): round 1 proposed a
commit-height-TTL cache; round 2 (skeptic) showed commit-height measures repo busyness, not
fact staleness, and that the panel's own worked example (the opencode/sandbox_dispatch
retirement) already broke that invalidation rule; round 3 (creative) collapsed the
invalidation into a single path-based rule and cut a redundant per-tick write-budget field;
round 4 (skeptic) found three real gaps -- audit only checked pool meta-health, not
per-slot correctness; `stall-diagnosis`/`dispatch-note` facts collided with `TICK_POLICY`'s
existing "not a new file" rule for incidents; path-based staleness silently breaks for
facts with no file to scope to -- and fixed them by narrowing scope and hardening the read
path; round 5 (pragmatist) cut the SKILL.md-seeding idea (wrong kind, already preserved
elsewhere) and settled on an empty launch.

**Caveat:** round 5 claimed "every code/line citation in it checks out against the live
repo." That claim was checked directly (not taken on faith) against the current
`drive_tick.py` and `board-lint.py` on `main` -- the `board-lint.py:57-58` citation was
correct, but every `drive_tick.py` citation the panel produced was off by roughly 20-21
lines (e.g. `safe_signal` cited at 111, actually at 132; `AUDIT_POLICY` cited at 288-299,
actually at 309-320). All line citations in this document have been corrected against the
live file; treat any *uncorrected* line citation from a future panel run the same way --
verify, don't trust the panel's own "checks out" claim.
