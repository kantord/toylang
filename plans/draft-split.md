# Retiring draft.md: the section inventory

Commissioned by kantord/toylang#120, executing the `draft-split-decision` ruling: draft.md
stops being the living reference -- the docs site is -- and each section either becomes a real
docs page, ADR, or tracked question, and is then deleted from the file. Each row below owns
its sections' migration *and their deletion*. A row may leave adjacent material behind rather
than over-reach; `draft-md-cleanup-review`, blocked on every row here, reviews whatever
remains and in theory deletes the file.

Ground rules for every migration row:

- The docs site is the destination of record. Verify the claimed coverage before deleting
  anything: a section is deletable when its decisions are recorded (ADR or reference page)
  and its still-open threads have moved to the question tracker, which now lives in
  [plans/questions.md](questions.md).
- Not all prose deserves to survive. Superseded reasoning, agent/user TODO dialogues, and
  duplicated argument can die with the section; what must survive is decisions, their
  load-bearing rationale, and open questions.
- Sections are named by heading, not line number; the file shrinks as rows land.

## The rows

### draft-questions-home

Sections: "Open questions", "Question detail".

The keystone row, and why every other migration soft-blocks on it: sections push their
embedded open threads *somewhere*, and this row decides where.

Landed: the tracker is [plans/questions.md](questions.md), one file, table and per-question
detail, numbering unchanged. Per-question GitHub issues lost on the tracker's own rule --
settled entries stay so decisions are not relitigated -- because a closed issue is not
something the next reader checks a table against for completeness. A later row that wants a
question worked on can still open an issue for it; the tracker entry stays the record. Push
open threads there by adding a numbered question at the bottom; link to it as
`plans/questions.md#q<N>-<slugged-heading>` from wherever the thread came from.

### draft-intro-migration

Sections: "What this is", "Two guiding principles", "Values", "Two worked programs",
"Non-goals".

The language's identity and vision. The docs site has no overview page: tutorials, guides,
reference, and ADRs all assume the reader knows what the language is. Destination: one
overview/vision page (placement per site conventions). The two worked programs use unbuilt
features (`fold {} with`, `^.`, `on "]q"`); they are vision, not documentation, and belong on
the same page labeled as such. The jaq-fork history in "What this is" can compress to a
sentence; ADR 0002 already carries the backends-as-falsifiers framing.

### draft-core-model-migration

Sections: "The core idea: two layers", "Cardinality is part of the type", "Why
cardinality-in-the-type is the safety mechanism".

The settled core -- two layers, reflect/reify, the cardinality table, base functors, the
safety argument -- is stated nowhere on the docs site as a connected story; the streams guide
and ADR 0001 assume it. Destination: a concepts guide. The long TODO/RESPONSE threads inside
"Cardinality is part of the type" (multidimensional vectors Q9, tensor/rectangularity Q17,
copy-on-write Q10, select-copy Q14) are open-question material, not doctrine: move to the
tracker, don't enshrine.

### draft-access-model-migration

Sections: "Functions are unary", "Field access is a lens", "PROPOSAL: every dimension gets a
spec".

Largely built and documented (`reference/syntax/functions.md`, `reference/operators/specs.md`,
`projection.md`, `unwrap.md`); verify, salvage missing rationale (the bidirectional-checking
argument, the spec vocabulary's derivations), delete. The lens trait sketch (`set`, `path` --
write and path-witness halves) is unbuilt future design: tracker.

### draft-numbers-operators-migration

Sections: "DECIDED: Int is 32 bits and wraps" (including its conditional-expression,
output/unlines, and six-backends subsections), "DECIDED: Float is JavaScript's double",
"DECIDED: equality on a composite is structural, and stops at a Vec".

The best-covered cluster: ADRs 0006, 0007, 0010 plus `reference/types/int.md`, `int64.md`,
`operators/arithmetic.md`, `conditional.md`, `comparison.md`, and ADR 0002 for the
backend-audit framing. Mostly a verify-and-delete row; anything the ADRs lack (the
literal-width rule's Go story, the ordering-still-disagrees caveat on composites) moves into
the matching reference page first.

### draft-records-migration

Sections: "DECIDED: records can be built, and a record is how several arguments travel",
"DECIDED: record fields keep their declared order", "DECIDED: record field order is not type
identity".

Destination: `reference/types/record.md` and `reference/syntax/functions.md` (the
unary-functions/record-argument story is shared with draft-access-model-migration; whichever
row lands second reconciles). The punning refusal and its reason deserve to survive.

### draft-streams-migration

Sections: "DECIDED: a minimal cut of streaming input, pull-based, one new keyword", "DECIDED:
one stdin source is the destination", "DECIDED: `inputs`, eager, not an answer to Q1 either",
"DECIDED: `jsonlines`, and the jq tutorial reproduces in full", "DECIDED: Stream is the
effect layer, typed".

ADR 0001, `guides/streams.md`, `reference/sources/*`, `reference/types/stream.md`, and
`builtins/jsonlines.md` carry the current state. Much of these sections is implementation
narrative already echoed in research-log; the durable parts are the rules (sources, mappers,
exits, linearity, second-class) and the pull-not-push argument. The three stdin keywords are
officially transitional (`stdin-syntax-design` / `stdin-redesign-build` rows), which is a
reason to migrate the *decisions* and drop the narrative now, not to wait.

### draft-calls-modules-migration

Sections: "DECIDED: `f x` reads as `f(x)`, but only where an expression begins fresh",
"DECIDED: a rudimentary module system, one prelude file and `pub`".

Destination: `reference/syntax/functions.md` / `programs.md` for bare application (the
same-line rule, the `-` and capitalized-callee exclusions), `reference/prelude/` or a small
modules page for the prelude story. Q36's remainder (imports, privacy) is already in the
tracker.

### draft-matching-migration

Sections: "Pattern matching is decoding", "One combinator algebra for trees, strings, and
streams", "DECIDED: match arms compose with `or`, and a guard chain may be honestly partial",
"DECIDED: enums, nominal and JSON-native".

The ratified layer (arm syntax, totality hybrid, enums, generic enums, Opt-as-enum, matchers
as first-class values) is covered by `guides/matching.md`, `guides/enums.md`, ADR 0009, and
the #47/#62 issue records; verify and delete. The unratified layer -- the codec/decoder
threads, combinator-algebra claims (Q30), string patterns (Q31), deep matching (Q28) -- is
tracker material. Soft-blocked on the two matcher decide rows because the conceptual model is
being re-grilled (gh:122); migrating mid-flux invites rework, but nothing here is
hard-blocked.

### draft-prototype-findings-migration

Sections: "What the prototype showed".

Every subsection already ends in a research-log link, and the statuses it hedges about
("still yours to move") have since been moved -- Q1 settled, records built, `lines` replaced.
Verify each finding is genuinely in research-log or superseded, then delete; this section is
the file's most fully-paid-out one.

### draft-str-adr

Sections: "Strings are where platform independence actually costs something".

The string-type-spike (gh:100, `plans/string-type-spike.md`) concluded the contract already
exists and needs *recording*, not deciding: scalar-value sequence, codepoint ordering, I-JSON
wire form, surrogates refused at edges. Write that ADR per the spike's recommendation 1, then
delete the section -- its three-options analysis is superseded by the spike's fuller one.
Soft-blocked on `lines-utf8-edge` (gh:102) only because the ADR could pin that edge in the
same breath if it lands first.

### draft-mutation-migration

Sections: "Mutation", "Mutation as an optimization: privileged and shared references".

Hard-blocked, honestly: the cells sketch and privileged/shared references await
`mutation-semantics-design`, and the UNDECIDED record-forming update subsection is exactly
`binary-op-multiplicity-design`'s Q3. Once both decide rows land, their outcomes get recorded
(ADR or reference) and both sections die. Until then the sections are the best statement of
the open design and stay put.

### binary-op-multiplicity-design (decide)

Sections: the "UNDECIDED: what to call the record-forming update" subsection of "Mutation";
question detail Q2 and Q3.

The oldest open question with no board row: what a binary operator over two multi-valued
operands means (cartesian, zip, or explicit `cross`/`zip`), and with it the record-forming
update's spelling (leaning B: require One on the right, fork explicitly). Q2 gates Q3
explicitly, the Vec-equality refusal, and the `[.[] + .[]]` divergence table entry. Outcome:
ADR plus tracker updates; the draft subsection then migrates via draft-mutation-migration.

### search-and-fold-design (decide)

Sections: "Query is search", "Single-pass composition".

Neither is built and neither is ratified: the search vocabulary (`,` as choice point, `empty`,
`first` as cut, `..` traversal and Q7's ordering promise, `//`'s fate after arm-`or` retired
it) and the applicative fold block (`fold xs { ... }`, the reduce/fold split). Both sections
read as settled but no decision record exists, and pieces have drifted (arm chains took `//`;
`sum-max-reductions` and `first-search-cut` rows are carving off pieces). Grill what remains,
record it, then migrate/delete. Soft-blocked on `first-search-cut` (same territory: first as
cut vs the full search story).

### offload-boundary-design (decide)

Sections: "Backends, vectorization, and the offload boundary" (including the dense tensor
subsection and the stdin-typing TODO at its end, which the streams decision has since
answered).

The performance thesis: kernel admissibility by cardinality, the batch-invariance law
(Q20/Q21 lean on it), the parallel-basis primitive set (Q23), CPU vectorization, tensors
(Q17-Q19). The backend-choice subsection is already ADR 0002/0005 material and can be
verified against them. The rest is an argued-but-unratified thesis whose test is benchmarks
that don't exist yet -- hence soft-blocked on `benchmark-goals`, which decides what the
thesis even owes. Grill, record what's ratified (likely as one design doc plus tracker
updates), migrate/delete.

### draft-md-cleanup-review

Blocked on every row above. Review what is left of draft.md -- rows were allowed to leave
adjacent scope behind -- assign or delete the remainder, and if the file is empty, delete the
file and fix every reference to it (README, CONTEXT.md, research-log links, site).
