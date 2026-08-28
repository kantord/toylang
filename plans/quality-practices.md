# Adopting toy-browser's quality practices

What kantord/toy-browser runs that this repo does not, evaluated piece by piece against how
toylang actually works. Surveyed in full: its `.claude/` tree (the checks, the settings, the
code-style skill and its eleven lesson files), `clippy.toml`, the workspace lint config, and
the four ADRs that record why the machinery is shaped the way it is: 0005
checks-point-at-lessons, 0006 lessons-are-tested-by-trickery, 0008
a-budget-set-below-the-code, 0009 a-sinkhole-for-the-rare-exemption.

Nothing is installed by this proposal. Each piece ends in a recommendation; the decisions go
to the adoption session (the board's `quality-practices-adoption` row). Numbers about this
repo below were measured by running toy-browser's checks and thresholds against it, not
estimated.

## The shape of the thing being evaluated

toy-browser's machinery is one idea applied five ways: **checks emit findings named by kind,
each kind names a lesson, and a finding no lesson settles stops the agent and becomes a
question to a human.** Rules accumulate from decisions actually made, never guessed up front.
Concretely:

- A `PostToolUse` hook runs rustfmt on every written `.rs` file, silently.
- A `Stop`/`SubagentStop` hook runs a check script over the files the session touched:
  a file-length budget (inline tests measured separately), OKF frontmatter validation on
  lesson files, a sinkhole validator, and every clippy warning forwarded and filtered to
  touched files. Findings block the stop and point at lesson files.
- Budgets live in `limits.toml` and `clippy.toml`, only ever ratchet down, and are set low
  enough to fire ("a threshold nothing can reach teaches nothing" -- their ADR-0008).
- `#[allow]` anywhere is itself a finding; a justified exemption must move into a "sinkhole"
  file that a second check keeps honest by re-running the lint with `--force-warn`.
- Lessons are an [OKF](https://okf.md/) bundle of small linked playbooks, held to the same
  line budget as code, written only after an escalation settled the case.

The fit for toylang is good for a structural reason: development here runs through delegated
sessions off `plans/board.yaml`, each in its own worktree that carries `.claude/` with it.
Hooks are therefore the one quality mechanism that reaches every session automatically,
before `land-delegated-work` ever reviews the branch. And the worst operational problem
toy-browser recorded -- parallel agents seeing each other's half-finished files in findings,
because `git status` scopes to a shared tree (their ADR-0005, "seven blocked turns") --
mostly cannot happen here, because each delegated session owns its worktree.

One adaptation is load-bearing enough to state before the pieces:

### Touched-file scoping must survive frequent commits

toy-browser's check defines "files this session touched" as `git status --porcelain`:
whatever is uncommitted when the session stops. That matches their flow, where work sits
uncommitted at stop time. It does not match ours. AGENTS.md has sessions committing per
step with provenance lines, so a delegated session's tree is *clean* at Stop, and the check
as written would report nothing, every time -- the whole mechanism silently no-ops. The
caused/inherited marker breaks the same way: comparing against HEAD, a file grown over
budget across five commits reads "inherited" from the last one.

The adaptation is one line of git: touched = changed since `merge-base main HEAD`, plus
anything uncommitted; "before" for the caused/inherited split is the merge-base blob, not
HEAD. Everything else in the script transfers unchanged.

## The pieces

### 1. The format hook: adopt as-is

`.claude/checks/format.sh`, wired to `PostToolUse` on Edit|Write: runs
`rustfmt --edition 2024` on the file just written, fixes silently, never reports (a mid-edit
file that does not parse is formatted on the next write). Their comment states the principle:
formatting has one right answer, so a check that would only ever teach "run rustfmt" should
just run rustfmt. No lesson, no finding, no decision to make.

Nothing about toylang changes this. Adopt verbatim.

### 2. The Stop-hook check runner: adopt, with the scoping fix

`.claude/checks/run.sh` is the centerpiece: exit 2 with findings on stderr blocks the agent's
stop and feeds the findings back to it, each naming its kind, its detail, and the lesson path.
It honors `stop_hook_active` so it cannot loop, dedupes clippy's per-target repeats, and
scopes everything to touched files so an agent only hears about what it worked on.

Adopt the runner and the `Stop` + `SubagentStop` wiring, with:

- the [merge-base scoping fix](#touched-file-scoping-must-survive-frequent-commits) above;
- `OWNED` patterns for this repo: `*.rs`, `*.sh`, and markdown as [piece 5](#5-the-prose-budget-adopt-narrowed)
  narrows it. There is no `*.js` here to own. Corpus YAML stays out: case shape is already
  enforced harder than a hook could (unknown keys are errors, `tag_corpus.rs` rewrites
  `node_types` every run);
- the OKF frontmatter check widened to `research-log/*.md`, which is already an OKF bundle
  by declared intent (`type: Lesson`, maintained by the research-log skill) that nothing
  currently validates. This check generalizes to it for free.

### 3. The file-length budget: adopt, starting above the code

toy-browser budgets whole files (`max_file_lines = 320`, prose included, inline tests
measured separately so relocating tests cannot pass as restructuring). The number only
ratchets down, and findings carry caused/inherited/new-file so a session is never asked to
haul a 600-line refactor into a two-line change.

Measured against this repo at their 320: ten of eighteen `src/` files are over, and so is
nearly every file a compiler-touching session would open -- `emit_llvm.rs` 1666,
`check.rs` 1529, `emit_rs.rs` 1116, `emit_go.rs` 969, `parse.rs` 965, down through
`emit_jq.rs` 476. (No `src/` file has inline tests; the suite lives in `tests/`, so the
inline-test carve-out is dormant here but harmless to keep.)

toy-browser could start at 400 because three files were over. Starting there here would open
*every* session with the inherited-debt conversation, and a finding that fires on every
session teaches agents to skip reading findings -- their own argument, from the other
direction (ADR-0008: a threshold normal work trips "teaches people to skip reading it").
Recommend starting at **1000**, which names the three right first conversations
(`emit_llvm.rs`, `check.rs`, `emit_rs.rs`), and ratcheting down by deliberate commits as
splits land, per their `limits.toml` discipline. The floor is for the adoption session; 400
looks reachable, 320 unproven.

One seven-backend caution for the eventual lessons: the emitters are parallel in structure,
so a split shape decided for one will be applied to all seven. That is high leverage and
high blast radius -- the first emitter split deserves a grilling session, not an agent's
improvisation, and the lesson it produces pays for itself six more times.

### 4. The clippy lints: adopt all three, thresholds staged

toy-browser turns on three lints in `[workspace.lints.clippy]` (so plain `cargo clippy`
sees them; the hook adds nothing of its own) with thresholds in `clippy.toml`:
`cognitive_complexity` at 5, `too_many_lines` at 40, `fn_params_excessive_bools` at 1.
Their reasoning transfers cleanly because it is about agent behavior, not their codebase:
four independent runs put competent fresh code at complexity 4 or under, so 5 sits one notch
above where ordinary work lands. `too_many_lines` exists because a long *straight* function
is invisible to both the complexity and the file budget. The bools lint at 1 names the cause
(two callers sharing one body, paid for in flags) that complexity only sees as a symptom.

Measured here at their thresholds: **27** cognitive-complexity findings (worst 25/5, in
`emit_go.rs`; then 18s in `emit_lua.rs` and `emit_rs.rs`; the median is 7),
**54** too-many-lines findings (worst is `check.rs`'s 445-line checker body, then a 266-line
function in `emit_llvm.rs`; 15 are over 100), and exactly **one** bools finding --
`run_jq(source, has_value, raw, uses_lines, feed)` in `src/lib.rs:365`, three bools, though
each is a derived fact about the program rather than a caller identity, which is precisely
the carve-out their fn-params lesson ends on. Today `cargo clippy` is clean, because none of
these lints are on by default.

Recommend adopting all three lints in the manifest, with starting thresholds set where the
standing findings are countable rather than wallpaper: **cognitive-complexity 10** (10
findings: the five 11s and up, all in emitters and `parse.rs`), **too-many-lines 100** (15
findings), **max-fn-params-bools 1** (their number; it fires once and that once is worth a
look). Ratchet toward 5/40 as the debt burns down; the ratchet direction rule -- down by
deliberate commits, never up to clear a finding -- adopts verbatim. Their warning about the
metric adopts too: clippy counts `if`/`else`/loops/guards but not match arms, so the score
ranks nothing. That matters more here than there, since emitters are mostly wide matches
over TIR -- which is exactly why our 27-at-threshold-5 is not the emergency it sounds like,
and why the lesson must say "look at the shape, not the number".

### 5. The prose budget: adopt, narrowed

toy-browser's `OWNED` includes `*.md`, deliberately: their lessons and docs are held to the
same budget as code, "which is what stops a lesson growing into a wall nobody reads".

For this repo that is right for `research-log/` (notes are already small; the index is 97
lines), `plans/` (largest is 108), `.claude/skills/` (largest 88), and the root docs
(AGENTS.md 281, CONTEXT.md 148) -- all comfortably under even 320, so the budget merely
holds the line.

`draft.md` is the exception: 2883 lines and the center of the design workflow. It is,
honestly, the wall of prose the budget exists to prevent, and splitting decided material out
of it might be genuinely good. But that is a restructuring of the design conversation itself
-- a decide row for the board, not something a line-count hook should force on whichever
delegated session next touches the draft. Recommend excluding `draft.md` by name, with a
comment saying it is excluded because its shape is an open decision, and filing that
decision as its own board row rather than leaving the exclusion permanent by default.

### 6. The lesson protocol: adopt the protocol, not the lessons

The code-style SKILL.md defines the loop: read the lesson the finding names, follow its
links, apply the matching case, and *escalate anything the lessons do not settle* -- by
`AskUserQuestion` with real alternatives and costs, never by improvising, never by prose
that auto-mode scrolls past. The settled outcome is written back as a line in an existing
case, a one-sentence combination note, or (only when genuinely new) a new small linked node.
Suppression is escalated like anything else and "almost never comes back yes".

Adopt the protocol. It is the piece that turns the checks from linting into accumulated
judgment, and its escalation path maps onto machinery this repo already has: a grilling
session is toylang's native unit for exactly these decisions, and the board is where one
gets scheduled. Two adaptations:

- **Escalation joins the board flow.** A delegated session that hits an unsettled finding
  asks via `AskUserQuestion` (sessions are interactive; the user can take any of them over),
  but also records the open question in its report, so the drive loop's coordinator can turn
  a parked session into a decide row instead of reading it as a stall. This is toy-browser's
  own subagent rule ("a subagent cannot ask; the agent that spawned it turns the report into
  a question") lifted one level, to sessions and the coordinator.
- **Start the bundle empty.** Do not copy toy-browser's eleven lesson files. They cite
  toy-browser code (`realm.rs`, `write_node`, `prelude.js`) as worked examples, and more
  importantly they record decisions *that repo* made after escalations *it* had; importing
  them wholesale manufactures a history of decisions nobody here made, which is the exact
  failure their ADR-0008 flags ("the lesson exists before any agent was blocked into writing
  it... therefore less trustworthy"). Ship the index and SKILL.md with zero lessons; let the
  first findings force the first grillings. Where a toy-browser lesson contains a
  repo-independent insight -- the over-parametric one ("a parameter passed through unchanged
  is a caller's identity, smuggled in as data") is the standout, and it speaks directly to
  `run_jq`'s three bools -- cite it as a source when the corresponding lesson gets written
  here, per AGENTS.md's citing rule.

### 7. The sinkhole rule: adopt in principle, last in order

Their rule: `#[allow]` beside the code it excuses is a finding; a justified exemption moves
into a sinkhole file (one `#![allow]`, one lint, a `//!` block arguing why, and every
function in it must still trip the lint under `--force-warn`, so freeloaders are evicted
mechanically). The friction is the mechanism -- an exemption costs a code move a reviewer
sees.

This is the piece least urgently needed here and the only one with day-one casualties:

- `tests/support/mod.rs` opens with `#![allow(dead_code)]`. The sinkhole validator would
  reject it -- `dead_code` is a rustc lint, which the rule refuses on the grounds that rustc
  lints usually have a real fix (their ADR-0009 resolved the same allow by deriving
  `Serialize` so the fields were genuinely read). Resolving this one is a small prerequisite
  task, not a blocker.
- `src/emit_rs.rs` *emits* `#[allow(non_camel_case_types)]` and `#![allow(dead_code)]` into
  generated Rust programs, as string literals. Their greps happen to skip these only because
  they anchor at line start and the literals sit inside `format!` calls. That anchor is
  load-bearing here in a way it is not there; the adopted script should say so in a comment,
  because the next person to "improve" the grep will hit it.

Recommend adopting it, but only after the clippy lints have been in place long enough for
the first real exemption pressure to exist. A sinkhole rule with no lints that tempt anyone
is machinery without a purpose, and the standing findings from piece 4 will generate the
temptation soon enough.

### 8. Testing lessons by trickery: do not adopt the ritual

Their ADR-0006: a lesson is trusted only after an agent that was never told it existed
found it via the hook and followed it, arranged by giving a subagent a minimal unrelated
task on an over-budget file. The method is sound and their execution notes are worth
keeping (never test in the session that wrote the hook -- settings are read at session
start; run one at a time; revert the trigger).

Here it is unnecessary as a ritual, because the conditions it manufactures occur naturally:
ten files stand over any plausible budget, every delegated session is exactly the
fresh-agent-with-no-context the trick simulates, and the board guarantees a steady supply
of small real tasks that touch big files. The first few delegated sessions after the hooks
land *are* the test, on real work instead of throwaway triggers. Recommend writing their
session-start caveat into the adoption commit message (whoever installs the hooks will not
see them fire in that session) and otherwise letting reality run the experiment.

### 9. What not to adopt, and what already exists

- **Their lesson content**, per [piece 6](#6-the-lesson-protocol-adopt-the-protocol-not-the-lessons).
- **`*.md` in OWNED unconditionally**, per [piece 5](#5-the-prose-budget-adopt-narrowed).
- **Their starting thresholds as-is**, per pieces [3](#3-the-file-length-budget-adopt-starting-above-the-code)
  and [4](#4-the-clippy-lints-adopt-all-three-thresholds-staged): right philosophy, wrong
  tree. Their numbers assume a near-clean baseline; ours has 82 standing clippy findings at
  their thresholds, and a check that fires constantly is a check that stops being read.
- **CONTEXT.md-style vocabulary, ADRs, the justfile** -- toy-browser has them, this repo
  already has its own. Nothing to do. One genuinely small gap: this repo's justfile has no
  `clippy`/`check` recipe, worth adding alongside piece 4 so humans and hooks run the same
  incantation.

## Order of introduction

Each step is separately shippable and separately revertable; nothing later depends on a
threshold chosen earlier.

1. **Format hook** (piece 1) with `settings.json` carrying only `PostToolUse`. No decisions,
   no findings, immediate value in every delegated session.
2. **Check runner + file budget + prose budget + OKF check + the skill** (pieces 2, 3, 5, 6)
   together, since the runner is inert without a check and the findings are dangerous
   without the skill's protocol. Decisions needed from the adoption session: the starting
   file budget (1000 proposed), the `draft.md` exclusion, and the escalation wording.
3. **Clippy lints** (piece 4): manifest lints, `clippy.toml` at 10/100/1, a `just clippy`
   recipe. Decision needed: the starting thresholds.
4. **Sinkhole machinery** (piece 7), after the `tests/support/mod.rs` allow is resolved and
   once the lints have produced their first real exemption argument.
