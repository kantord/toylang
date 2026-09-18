# Open questions

The design question tracker. It lived in draft.md until the [draft-split
plan](draft-split.md) moved it out; the numbering is unchanged, so Q17 still means what it
meant wherever an ADR, a research-log note, or a plan cited it.

Status is one of OPEN (no preferred answer), LEANING (a preferred answer exists but is not
committed), BLOCKED (waits on another question), or SETTLED (answered, and the answer is
written down somewhere a reader can reach from here). Add new ones at the bottom and keep the
numbers stable, since other documents cite them.

Settled questions stay in the table. A tracker that only lists what is unresolved cannot be
checked for completeness, and the settled entries are what stop a decision being relitigated.
What a settled entry stops carrying is the argument: once an ADR or a decision section exists
to point at, the detail collapses to a line and a link, and that record answers the question
from then on. Where no such record exists yet the entry keeps its detail, because collapsing
it would delete the only copy.

| # | Question | Status |
|---|---|---|
| [Q1](#q1-streams-first-class-values-or-evaluation-level-multiplicity) | Streams: first-class values, or evaluation-level multiplicity? | SETTLED, evaluation-level and typed: `Stream<T>` is the effect layer's type, not a value type |
| [Q2](#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit) | Binary operators over two multi-valued expressions: cartesian, zip, or explicit? | SETTLED (wizard submission, 2026-09-08): option A, cartesian default; built 2026-09-18, every operator but `+` runs cartesian over two Vecs on all seven backends |
| [Q3](#q3-what-symbol-replaces--for-the-record-forming-update) | What symbol replaces `=` for the record-forming update? | RATIFIED (multiplicity-and-offload round 2): option B, keep `=` and require a `One` on its right, so forking is explicit; not built |
| [Q4](#q4-can-the-type-express-ordering-over-heterogeneous-streams) | Can the type express ordering over heterogeneous streams? | OPEN in the general case; the shape is decided (ADR 0008: Kleene patterns in effect position), enums supply tagged alternation, and the `Seq<Head, Rest>` spelling is built as a checker type (`seq-type-primitive-build`); runtime-pair emission is still to come |
| [Q5](#q5-stream-lowering-strategy-across-the-three-backends) | Stream-lowering strategy across the three backends | OPEN in general; all seven backends stream the fused pipeline shape, so only lowering beyond that shape remains |
| [Q6](#q6-does-a-reconciler-belong-in-the-language-or-a-library) | Does a reconciler belong in the language or a library? | OPEN |
| [Q7](#q7-does--promise-depth-first-order-or-only-the-set-of-nodes) | Does `..` promise depth-first order, or only the set of nodes? | RULED (2026-09-01, confirmed 2026-09-07): promises depth-first order, jq-compatible; the columnar-fast-path research brief landed and recommended closing without reevaluation. `..` itself is not implemented |
| [Q8](#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization) | Is vectorizability visible in the type system, or a silent optimization? | CONFIRMED silent (offload-boundary-design round 2): no type-level effect; discoverability is the opt-in `--explain-offload` diagnostic, built |
| [Q9](#q9-are-vectors-multidimensional-with--as-projection) | Are vectors multidimensional, with `[]` as projection? | OPEN, may merge with Q2 |
| [Q10](#q10-is-uniqueness-analysis-in-scope-for-deciding-when-a-lens-materializes) | Is uniqueness analysis in scope, for deciding when a lens materializes? | SETTLED yes, compiler-internal (mutation-semantics-design, 2026-09-17): v1 strict single-use, refusing across call boundaries, landing one backend at a time; Rust landed |
| [Q11](#q11-how-does-the-querytransformation-split-manifest-in-the-type-system) | How does the query/transformation split manifest in the type system? | SETTLED |
| [Q12](#q12-on-a-type-mismatch-does-field-access-error-yield-null-or-something-third) | On a type mismatch, does field access error, yield null, or something third? | SETTLED |
| [Q13](#q13-does-the-layer-shift-run-only-one-way-with-no-value-to-effect-operator) | Does the layer shift run only one way, with no value-to-effect operator? | LEANING yes, and now load-bearing: born-at-sources, dies-at-exits is the `Stream<T>` typing rule |
| [Q14](#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy) | Does `select` return a masked view, a selection vector, or a copy? | RULED (offload-boundary-design round 5, 2026-09-07): `select` keeps returning `Vec`, backed by a lazy {source, mask-or-selection} pair that compacts on the first strong reference; per-backend build rows are on the board |
| [Q15](#q15-backend-llvm-via-inkwell-cranelift-or-both) | Backend: LLVM via inkwell, Cranelift, or both? | SETTLED, LLVM via inkwell, built and running |
| [Q16](#q16-string-representation-given-wtf-16-on-the-js-target) | String representation, given WTF-16 on the JS target | SETTLED by ADR 0011 (2026-09-07): the third option, no length, no indexing, no splitting; `chars` is the only decomposition and codepoint order is the law |
| [Q17](#q17-is-there-a-dense-tensor-type-constructed-explicitly) | Is there a dense tensor type, constructed explicitly? | SETTLED (dense-tensor-type ruling, 2026-09-08): no separate type -- `Vec` itself is the tensor-capable type (rectangular, shape-checked), constructed via `tensor(n; m)`, no width commitment |
| [Q18](#q18-does--on-a-rank-2-tensor-yield-rows-or-scalars) | Does `.[]` on a rank-2 tensor yield rows or scalars? | SETTLED rows; transpose/column-access view RULED in scope (dense-tensor-type ruling, 2026-09-08) and built on Go and Rust only, with no corpus case yet |
| [Q19](#q19-how-are-nulls-carried-in-a-dense-typed-buffer) | How are nulls carried in a dense typed buffer? | SETTLED (dense-tensor-type ruling, 2026-09-08): hard-fail only, no bitmask -- reverses the earlier Arrow-bitmask leaning |
| [Q20](#q20-how-are-blocking-operators-sort-group_by-joins-classified) | How are blocking operators (`sort`, `group_by`, joins) classified? | SETTLED, a trait with no lawful stream instance |
| [Q21](#q21-what-guarantees-batch-size-is-unobservable-over-a-batched-stream) | What guarantees batch size is unobservable over a batched stream? | RATIFIED (offload-boundary-design rounds 4 and 5): a declared associative combiner is required at every batch boundary, and batches are an opaque `Batch<T>` with no operations, made by `batch(s, n)` with `n` a runtime hint; not built |
| [Q22](#q22-are-dense-and-masked-vectors-distinguishable-in-the-type) | Are dense and masked vectors distinguishable in the type? | RULED with Q14 (offload-boundary-design round 5): not distinguishable in the type; the mask lives behind `Vec` |
| [Q23](#q23-what-primitive-set-is-the-standard-library-defined-over) | What primitive set is the standard library defined over? | RATIFIED option A (offload-boundary-design round 3): the full parallel basis (map, scan, reduce, gather, scatter, and segmented forms); `fold` and general recursion are convenience leaves |
| [Q24](#q24-are-compile-time-macros-a-first-class-concept) | Are compile-time macros a first-class concept? | OPEN, not yet evaluated |
| [Q25](#q25-does-the-language-have-union-types) | Does the language have union types? | PARTLY SETTLED: closed nominal sums exist (enums); anonymous structural unions remain an absence |
| [Q26](#q26-is-jsxs-children-slot-a-closed-per-site-union-or-an-open-one) | Is JSX's children slot a closed per-site union, or an open one? | OPEN, deliberately deferred to last |
| [Q27](#q27-does-pattern-matching-need-a-separate-matcher-type-distinct-from-result) | Does pattern matching need a separate `Matcher` type, distinct from `Result`? | SETTLED |
| [Q28](#q28-does-deep-matching-need-cross-match-unification-of-logic-variables) | Does deep matching need cross-match unification of logic variables? | OPEN |
| [Q29](#q29-what-is-the-default-discriminant-convention-for-a-derived-enum-codec) | What is the default discriminant convention for a derived enum codec? | SUPERSEDED: the enum decision made the single-key wrapper the value itself, not a codec default |
| [Q30](#q30-do-the-base-functor-generics-double-as-parser-combinators-across-trees-strings-and-streams) | Do the base-functor generics double as parser combinators, across trees, strings, and streams? | LEANING yes, implementation split still open |
| [Q31](#q31-does-a-friendlier-string-pattern-language-belong-in-the-language-and-what-regex-flavor-does-it-extend-to) | Does a friendlier string-pattern language belong in the language, and what regex flavor does it extend to? | OPEN |
| [Q32](#q32-does-the-dimension-model-subsume-the-effect-layer) | Does the dimension model subsume the effect layer? | OPEN, and it may dissolve Q13 rather than answer it |
| [Q33](#q33-does-a-spread-slot-in-a-call-give-partial-application) | Does a spread slot in a call give partial application? | RULED option C (partial-application-system-design, 2026-09-07): array-like and struct-like partial application are one mechanism once functions are first-class, and it must always be syntactically explicit; no build row exists yet |
| [Q34](#q34-do-named-types-exist-and-is-a-name-an-alias-or-an-identity) | Do named types exist, and is a name an alias or an identity? | OPEN for records; enums decided identity for themselves, and `type X = ...` shipped as a pure alias (docs/reference/types/alias.md), so the declaration form exists and only the identity half remains |
| [Q35](#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return) | What are stdout and stderr, and does a program write or return? | RULED in three steps (2026-09-07): opaque plumbing, then a merged stream where each item is tagged by origin, spelled as the prelude enum `PipeLine`; `pipe_through` builds that on Rust. Still open: how stdin and stdout are split |
| [Q36](#q36-does-a-real-module-system-need-imports-multiple-files-and-enforced-privacy) | Does a real module system need imports, multiple files, and enforced privacy? | OPEN for named imports and exports; file-scoped privacy and `@(path)` module routing are built (docs/reference/syntax/modules.md) |
| [Q37](#q37-how-do-floats-print-and-what-are-nan-and-infinity-in-a-json-shaped-value-model) | How do floats print, and what are NaN and Infinity in a JSON-shaped value model? | SETTLED (gh:145, built under gh:149): NaN and Infinity are values printed by name, `Float` division by zero is Infinity, and printing is ECMA-262 `Number::toString` on all seven backends; see docs/reference/types/float.md |
| [Q38](#q38-are-composites-ordered-at-all) | Are composites ordered at all? | OPEN |
| [Q39](#q39-is-a-timestamp-type-worth-a-third-numeric-type) | Is a timestamp type worth a third numeric type? | OPEN |
| [Q40](#q40-is-a-fieldk-lens-trait-part-of-the-design) | Is a `Field<K>` lens trait part of the design? | OPEN |
| [Q41](#q41-is-narrowing-a-record-to-a-subset-of-its-fields-an-operation) | Is narrowing a record to a subset of its fields an operation? | OPEN |
| [Q42](#q42-is-a-runtime-field-names-accessor-part-of-the-design) | Is a runtime field-names accessor part of the design? | SETTLED: `fields`, a builtin on every backend (docs/reference/builtins/fields.md) |

[Multidimensional vectors](#q9-are-vectors-multidimensional-with--as-projection) is the one
question still capable of changing [the two-layer
section](../docs/guides/cardinality.md#two-layers), now that
[streams](#q1-streams-first-class-values-or-evaluation-level-multiplicity) are settled, so it
should be resolved before that section is treated as stable.

## Question detail

### Q1. Streams: first-class values, or evaluation-level multiplicity?

SETTLED: evaluation-level and typed. `Stream<T>` is the effect layer's type, second-class and
consumed exactly once, not a value type. Recorded in
[ADR 0001](../docs/adr/0001-stream-is-the-effect-layer-typed.md).

### Q2. Binary operators over two multi-valued expressions: cartesian, zip, or explicit?

Cartesian (jq today), zip
(vectorized, with broadcast), or neither by default with explicit `cross` and `zip`?

Vec concatenation specifically is decided, without touching the rest of this question. Revised
2026-08-30 (oddities round, kantord/toylang#97): `+` on two `Vec`s of the same element type now
concatenates them -- the add-trait reading -- so `[1, 2] + [3]` is `[1, 2, 3]`, settling this
half of Q2 in favor of concatenation over cartesian or zip. The named builtin this superseded,
`concat(vv: Vec<Vec<T>>) -> Vec<T>`, existed specifically so adding it would not decide this
question (see [named functions kept an open question
open](../research-log/named-functions-kept-an-open-question-open.md)); it survives under the name
`flatten` for the case `+` cannot cover, an outer `Vec` whose length is not known at the call
site. The general question -- what any *other* operator means when both operands are Vecs --
is now SETTLED (wizard submission, multiplicity-choicepoint-http round, 2026-09-08): **option
A, cartesian default**. `Vec op Vec` becomes legal for every operator (except `+`, already
concatenation) and runs cartesian, matching jq's own default -- checked against a real jq
1.8.2 binary: `echo '{"a":[2,3],"b":[10,20]}' | jq -c '[.a[] * .b[]]'` gives `[20,30,40,60]`,
the full 2x2 outer product. No new builtin needed; `.a * .b` becomes legal and cartesian for
free. Built 2026-09-18 (board row binary-op-cartesian-build): `cartesian` in
src/check/mod.rs lowers `Vec op Vec` to the `map`/`flatten` nodes every backend already
emits, in jq's order (right operand outermost, pinned by the corpus case
`cartesian_order`), for every arithmetic and comparison operator. A Vec on one side only
was not in the ruling and stays a type mismatch; nothing broadcasts. Equality still stops
at a Vec inside a composite or inside another Vec, as the next paragraph says.

Composite equality is settled without touching it. `==` on a record or an enum compares
structurally, and is refused outright when the type carries a Vec anywhere inside it, so a
`Vec`-typed record field never quietly acquires whole-value semantics
([the equality decision](../docs/reference/operators/comparison.md)).

### Q3. What symbol replaces `=` for the record-forming update?

RATIFIED (multiplicity-and-offload round 2, binary-op-multiplicity-design): option B, `=`
stays and its right-hand side must be a `One`, so a forking update has to be spelled out.
The ratified spelling is written up, unbuilt, in
[the update operator page](../docs/reference/operators/update.md). The maintainer's caveat
from the same round applies to whatever binding syntax lands: no `$`-prefixed variable
names.

### Q4. Can the type express ordering over heterogeneous streams?

Subsumes the older cardinality-versus-
order thread, which asked whether the type system should track *how many* values an
expression produces or *in what order* the kinds arrive. Those turned out to be the same
question asked from two sides, so they are tracked here as one. The cardinality half is the
cheaper and more decidable option, and it catches the failure that actually bites, which is
multiplicity leaking into a position wanting exactly one value. The order half is what the
rest of this entry is about.
If a stream is "some `A`s, then some `B`s", can the type say so? One approach is *regular
expressions over types*, the same
algebra as string regexes but with types as the alphabet, so a pattern denotes a set of
permitted value-sequences: `Seq<A,B>` = `A* B*`, `Alt<A,B>` = `(A|B)*`, `Star<A>` = `A*`.
Three primitives suffice (Kleene's theorem), it is decidable, and unlike full session types
it needs no *linear types*, a discipline requiring each value be consumed exactly once,
which is powerful but infects the whole system. Unpacking one item is then the **derivative**
of the pattern: given that an `A` was just consumed, what remains? Open: how type tagging is
represented so the runtime and type-level guarantees stay symmetrical.

An exploration after [the streams decision](../docs/adr/0001-stream-is-the-effect-layer-typed.md)
committed to this shape without settling the open parts; it is recorded as
[ADR 0008](../docs/adr/0008-stream-protocols-are-kleene-patterns.md). The load-bearing findings:
the linearity objection above is obsolete, since the streams decision introduced exactly-once
consumption scoped to one second-class type, which is all a protocol type needs; `Opt`, `Vec`,
and `Stream` are already this algebra (`?` and `*` on the value side, `*` on the effect side),
so the pattern constructors extend the cardinality table rather than joining it; the empty
pattern is `Seq`'s unit, which makes a payload-free end cost nothing and collapses any
"stream plus end slot" primitive back into `Star<T>`; a closing message is `Seq<Star<T>, E>`
and several ways to end is `Alt` in terminal position, which types mid-stream failure and
makes errors structurally terminal. The soundness condition that keeps all of it second-class:
a `Stream` never appears under a value constructor, and may appear freely under pattern
constructors. Still blocking: union types ([Q25](#q25-does-the-language-have-union-types)),
discriminants ([Q29](#q29-what-is-the-default-discriminant-convention-for-a-derived-enum-codec)),
the matcher surface ([Q27](#q27-does-pattern-matching-need-a-separate-matcher-type-distinct-from-result)),
and the spelling question (patterns inside the constructor versus outer combinators).

### Q5. Stream-lowering strategy across the three backends

Lua has true coroutines, JavaScript has generators, native
has neither for free. Previously recorded as needing to be decided before any backend is
written, which turned out to be false: three backends exist without it, because nothing in
them streams. Then recorded as blocking any backend that *streams*, which the fused
`jsonlines(f(stdin | map(parse(.))))` loop showed is also false: all seven backends now stream that pipeline
shape as a plain read/transform/write loop, no coroutines or generators involved, because a
straight-line pipeline never needs to suspend. What the question still covers is lowering
beyond that shape -- a stream consumed by something that is not the tail of its own loop --
where the coroutine/generator/state-machine choice becomes real.

### Q6. Does a reconciler belong in the language or a library?

### Q7. Does `..` promise depth-first order, or only the set of nodes?

On a flat columnar layout,
"every node at every depth" is "every element of every buffer", which is embarrassingly
parallel. The dependent part is not the traversal but the *order*, since the flat layout is
not in depth-first order. jq promises the order. If this language only promises the set,
recursive descent becomes one of the cheapest operators rather than one of the most
expensive. This is not only a performance question: a jq-derived language that is fast
everywhere except recursive descent has a positioning problem, because `..` is one of the two
things people reach for jq to do.

**Ruled provisional** (signature-matching-and-search-cut round, 2026-09-01): `..` promises
depth-first order, matching jq. The maintainer flagged this for reevaluation once the
unordered/columnar-fast-path alternative has a real research brief comparing it against other
languages' recursive-descent semantics and use cases. That brief,
[recursive-descent-order-research](recursive-descent-order-research.md), landed 2026-09-07:
every precedent surveyed (jq, XPath, JSONPath) treats depth-first pre-order as load-bearing,
so the ruling stands with nothing left to re-ask. `..` itself is not implemented yet.

### Q8. Is vectorizability visible in the type system, or a silent optimization?

Reporting it
means a second effect alongside cardinality, and a visible fast-path/slow-path distinction in
signatures. Hiding it makes performance unpredictable in exactly the way this design is
trying to avoid. Note the two effects are orthogonal: `select` changes cardinality and
vectorizes fine as a mask, while `first` changes cardinality the same way and cannot
vectorize at all.

CONFIRMED silent (offload-boundary-design round 2): vectorizability is derived from
cardinality and never appears in a signature. Discoverability, the cost of hiding it, is
answered by the opt-in `--explain-offload` compiler diagnostic, which is built
(`offload-explain-flag-build`); editor hover is a wanted addition, not scheduled.

### Q9. Are vectors multidimensional, with `[]` as projection?

See the TODO and response in the
cardinality section. Unifies indexing with iteration, but disturbs the claim that there are
exactly two layer shifters, and per-dimension cardinality only describes rectangular data
while JSON is ragged.

### Q10. Is uniqueness analysis in scope, for deciding when a lens materializes?

Deciding when a projection lens can materialize instead
 of staying a view requires knowing no other reference to the source survives. That is
 linearity or uniqueness typing, the machinery deliberately avoided in [the ordering question](#q4-can-the-type-express-ordering-over-heterogeneous-streams).

SETTLED yes, compiler-internal (mutation-semantics-design, three forest rounds, 2026-09-17),
after the [mutation spike](mutation-semantics-spike.md) grounded the "provably one
reference" analysis: the first landing refuses across call boundaries, ships the strict
single-use v1 rule rather than the lazy-copy v2, and lands on all seven backends one at a
time, Rust first (landed). The v2 follow-up row exists already at the maintainer's request.

### Q11. How does the query/transformation split manifest in the type system?

SETTLED: it does not need to. `map` and `select` are the same operation with the multiplicity
stored in different places, so the split never reaches the type system; see [the two-layer
section](../docs/guides/cardinality.md#two-layers).

### Q12. On a type mismatch, does field access error, yield null, or something third?

SETTLED: something third. A type mismatch errors loudly rather than yielding `null`: an unknown
field is a compile error ([projection](../docs/reference/operators/projection.md)). Missing is
distinct from error: an out-of-range index yields an [Opt](../docs/reference/types/opt.md),
which prints `null` unless `!` insists otherwise ([unwrap](../docs/reference/operators/unwrap.md)).
The lens framing -- field access desugars to a lens --is unbuilt future design, tracked as
[Q40](#q40-is-a-fieldk-lens-trait-part-of-the-design).

### Q13. Does the layer shift run only one way, with no value-to-effect operator?

If effect multiplicity is born only from
 streaming sources and dies only into values through `[...]`, then no value-to-effect
 operator is needed, because degrading a `Vec` forgets its extent and buys nothing. LEANING
 toward yes. This decides [the streams question](#q1-streams-first-class-values-or-evaluation-level-multiplicity) with it, since the only thing that would break it is a value
 with genuinely unknown extent, which is what a first-class stream value would be. The streams
 question is now settled the compatible way, and this lifecycle -- born at `stdin` or `range`,
 dead at `collect` or a sink -- became the `Stream<T>` typing rule, so reversing this lean now
 means amending that decision too.

### Q14. Does `select` return a masked view, a selection vector, or a copy?

See the section on
 whether a value-layer `select` copies. A bitmask breaks `Vec`'s constant-time indexing
 promise, a selection vector keeps it and pays memory per survivor, and either view pins its
 whole source buffer alive.

RULED option B (offload-boundary-design round 5, 2026-09-07), the maintainer's own proposal:
`select` keeps returning `Vec`; behind the name is a {source, mask-or-selection} pair,
indexable through a lazily built selection vector or a mask with a popcount table, compacting
on the first strong reference and reusing the input's storage when that reference is unique
(the same condition [Q10](#q10-is-uniqueness-analysis-in-scope-for-deciding-when-a-lens-materializes)
uses). Whether the representation is shared with streams was answered in
`select-shared-mechanism-design`: `select` stays on its own subject-context dispatch. The
per-backend build rows are `select-lazy-materialization-build-*` in plans/board.yaml; Python
landed first.

### Q15. Backend: LLVM via inkwell, Cranelift, or both?

SETTLED: LLVM via inkwell, built and running. Recorded in
[ADR 0005](../docs/adr/0005-llvm-via-inkwell-for-the-native-backend.md).

### Q16. String representation, given WTF-16 on the JS target

The three
 options are WTF-16 everywhere, UTF-8 everywhere with the JavaScript-shaped API emulated, or
 designing the difference away by never exposing code-unit indexing or length. Only the third
 is cheap on both sides. It has to be decided early because it constrains the string API
 permanently.

SETTLED as the third option by
[ADR 0011](../docs/adr/0011-str-is-a-sequence-of-unicode-scalar-values.md) (2026-09-07): a
`Str` is a sequence of Unicode scalar values with no length, no indexing, and no splitting;
`chars` is the only decomposition and codepoint order is the ordering law. The ADR does not
cite this entry, which is why the status sat at OPEN for eleven days after it was accepted.

### Q17. Is there a dense tensor type, constructed explicitly?

SETTLED (vec-as-dataframe-type-research +
dense-tensor-type wizard ruling, 2026-09-08): no separate `Tensor` type -- `Vec` itself is the
tensor/dataframe-capable type (rectangular, shape-checked), not a distinct value kind.
Construction does not bundle a number-type commitment: `tensor(n; m)` is one stage, narrows
and shapes together, with no `@f32`-style width commitment (the earlier `@f32 | reshape(n; m)`
two-stage sketch is dropped). `Float` did not exist when this was ruled; it does now
([Float](../docs/reference/types/float.md)), which changes nothing about the ruling.
`tensor(n; m)` is not built: `tensor-constructor-build` has stalled twice and waits on a
convergence ruling.

### Q18. Does `.[]` on a rank-2 tensor yield rows or scalars?

SETTLED rows.
NumPy and APL both yield rows, which makes `map` rank-polymorphic and gives row sums as
`map(fold(add; 0))` with no new syntax; rank-1 yields scalars and `flatten` already covers
full linearization. RULED (dense-tensor-type wizard, 2026-09-08): a transpose/column-access
view is built now, alongside the tensor type, rather than deferred -- `.counts | transpose |
map(sum(.))` for per-column reductions. As of 2026-09-18 `transpose` emits on Go and Rust
only, has no corpus case, and is missing from the checker's builtin-name list, so it has no
reference page either; the `transpose-remaining-backends` row carries the rest.

### Q19. How are nulls carried in a dense typed buffer?

SETTLED (dense-tensor-type wizard,
2026-09-08): **hard-fail only**, no bitmask -- construction refuses on any null, the caller
resolves gaps (drop/fill) before construction, same as `@f32`/`tensor` already do. This
reverses the earlier Arrow-validity-bitmask leaning; the maintainer's own note called the pick
tentative ("not certain, but leaning this way"), so revisit if a real pipeline needs masked
nulls through construction. JSON has null and an `f32` buffer does not; NaN as a sentinel
collides with genuine NaN, which is why hard-fail (resolve before construction) is the only
representation now in scope.

### Q20. How are blocking operators (`sort`, `group_by`, joins) classified?

`sort`, `group_by` and joins are one value in and
 one value out, so the per-element cardinality mapping does not describe them. They need the
 whole input before producing anything and are parallelizable by other means. The
 kernel-admissibility result covers elementwise filters only, and this is the gap it leaves.

The answer: a blocking operator is one `Vec` in and one `Vec` out with no `Stream` instance
at all, so the checker refuses it on a stream subject rather than buffering behind the
program's back. `sort`, `sort_by`, `max_by`, `max`, and `sum` all carry the rule today; the
reference pages ([sort](../docs/reference/builtins/sort.md) and its neighbours) are the record.

### Q21. What guarantees batch size is unobservable over a batched stream?

Argued in [the admissible input set, and where batching comes from](../draft.md#the-admissible-input-set-and-where-batching-comes-from) rather than
here, since it arrived with that material. RATIFIED in two rounds of `offload-boundary-design`:
round 4 picked option B, the compiler requires a declared associative combiner at every batch
boundary, the same enforcement posture as `sort`'s element-type check; round 5 (2026-09-07),
after `batch-type-design-research`, made the batch itself an opaque `Batch<T>` with no
operations at all (no length, no indexing, no equality), produced by `batch(s, n)` where `n`
is a runtime hint rather than a type parameter, forced at the reader and at any whole-input
reduction and explicit everywhere else. That closes the observability hole by construction
instead of by a growing list of per-operation gates. None of it is built.

### Q22. Are dense and masked vectors distinguishable in the type?

Argued in [the admissible input set, and where batching comes from](../draft.md#the-admissible-input-set-and-where-batching-comes-from), where it
appears as the observation that a masked view and a dense buffer have different launch
preconditions. The same question as [what select returns](#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy), approached from
the layout side rather than the operator side, and ruled with it: the type does not
distinguish them, the mask lives behind `Vec`.

### Q23. What primitive set is the standard library defined over?

Argued in [the primitive set cannot be fold and recursion](../draft.md#the-primitive-set-cannot-be-fold-and-recursion). RATIFIED
option A (offload-boundary-design round 3): the full parallel basis now -- map, scan, reduce,
gather, scatter, and their segmented forms -- with `fold` and general recursion demoted to
convenience leaves, and future standard-library additions checked against the third
homomorphism theorem. The basis is a design commitment; the builtins that exist today are
listed under docs/reference/builtins.
### Q24. Are compile-time macros a first-class concept?

A macro would be a function that runs at compile time and transforms the compiler's own
representation of a program, which means that representation has to be a type the language
defines rather than an implementation detail the compiler happens to have. Fully compile-time, as
in Rust, with no runtime field.

The syntax idea is decorator-style, as in Python, where the same notation can attach either an
ordinary closure or a macro. That the two look alike is the point worth checking: it is either
the feature's main convenience or its main trap, since one runs when the program runs and the
other runs while it is being compiled, and the principle about writing crossings down applies to
that boundary too.

Not evaluated. Recorded so it is not rediscovered.

### Q25. Does the language have union types?

There is no sum type. `Alt<A,B>` appears only inside the regular-expressions-over-types sketch for
stream ordering, and `Json` stands in wherever a value might be several things, which makes it
the permissive escape hatch principle 1 says it should not be.

The gap surfaced from asking what `.[]` on a heterogeneous record would even produce. With a union
it is `Str | Int`; without one there is no answer. That question is settled on other grounds, but
the absence it exposed is not, and heterogeneous data is not a corner of a data language.

Related: an alternation over types is also what the ordering question needs, so these may be one
piece of machinery rather than two. Answered no by `stream-merge-tagged-design` (2026-09-07):
an ordinary closed enum already produces the tagged shape with no new type-system machinery,
so no `Alt<A, B>` is needed; the prelude's `PipeLine` enum is the shipped instance.

Partly settled by [the enum decision](../docs/guides/enums.md): closed nominal
sums now exist, and they serve both this question's motivating case (heterogeneous data) and the
ordering question's `Alt` (a stream of several message kinds is `Stream<SomeEnum>`). What
remains absent is the anonymous structural union, `Str | Int` with no declaration -- a
different feature with a different justification, still an absence rather than a decision.

### Q26. Is JSX's children slot a closed per-site union, or an open one?

Sketch: a creator function taking a `Record` of strictly-typed attrs (this is just
[`Field<K>`](#q40-is-a-fieldk-lens-trait-part-of-the-design) in the existing sense, no new machinery) plus a `Dimension` of children. The children slot needs
an element type, and that is where the interesting question lives.

React's `ReactNode` is open: any function shaped like a creator function is accepted, unconstrained
at the definition site. That is the same escape hatch [Q25](#q25-does-the-language-have-union-types)
already names and principle 1 rejects -- an unconstrained union is `Json` with a different label.

The alternative is closed and inferred per call site: each JSX expression's children type is
`Alt<T1, T2, ...>` built from whatever is literally nested there, checkable and exhaustive, with
no shared vocabulary required across call sites. The cost is fragmentation: a function written to
accept "a list of children" can only accept the exact union inferred at its own call site, not
children built elsewhere out of a different but compatible set of node kinds. Parametric
polymorphism over the union narrows that gap but does not close it.

Deliberately last: the right answer depends on the rest of the design (how this interacts with
[Q24](#q24-are-compile-time-macros-a-first-class-concept), and how JSX trees actually get passed
between functions in practice), not on this slot in isolation.

### Q27. Does pattern matching need a separate `Matcher` type, distinct from `Result`?

SETTLED yes: matchers are first-class, tagged, and or-composable, derived per type under the
capital name (kantord/toylang#47). See [the enums guide](../docs/guides/enums.md) and
[guides/matching.md](../docs/guides/matching.md).

### Q28. Does deep matching need cross-match unification of logic variables?

OPEN. `..` composed with a matcher already finds a shape anywhere in a tree without naming its
path, and `as` already binds one submatch to a name for reuse within the same arm. Neither needs
unification. A `..` rest-marker for matching a subset of a closed type's fields, leaving fields out being
a compile error by default,is likewise unbuilt sketch. What would: finding a node `A` and a
separate node `B` elsewhere such that `B`
refers to `A`, which is full Prolog-style unification with backtracking over bindings, not a
bigger version of `as`.

### Q29. What is the default discriminant convention for a derived enum codec?

SUPERSEDED: there is no derived codec picking a representation, because the representation
*is* the value. [ADR 0009](../docs/adr/0009-enums-are-json-native-single-key-wrappers.md)
records the decision, and why the tag-field and shape-matched alternatives lost.

The wider derived-codec thread -- a `Json -> T` decode,a `T -> Json` encode,and a `Str -> T`
parse, plus the JSON Schema projection, all falling out of one structural description -- is
deferred to the codec layer ADR 0009 names,with nothing settled there.The three codec
directions are one trait family picked by which types the codec sits between (the same shape as
[`Field<K>`](#q40-is-a-fieldk-lens-trait-part-of-the-design),and the decode-vs-encode split is the
total/partial split again: decode, and parse can fail,and encode cannot.

### Q30. Do the base-functor generics double as parser combinators, across trees, strings, and streams?

LEANING yes. `Seq`, `Alt`, `Star`, and `Opt` are already in the document as [the regex-over-types algebra](#q4-can-the-type-express-ordering-over-heterogeneous-streams)
and as the shape [the matcher decision](#q27-does-pattern-matching-need-a-separate-matcher-type-distinct-from-result) builds `Matcher<T>` from; naming them as parser
combinators only makes the precedent explicit (Hutton and Meijer; Wadler; parsing with
derivatives). OPEN: whether this is one trait with implementations that differ by receiver (a
parsed tree needs no backtracking, a string needs an actual parsing engine), the same shape as
[`Field<K>`](#q40-is-a-fieldk-lens-trait-part-of-the-design), and if so what law the implementations have to share. See
[One combinator algebra for trees, strings, and streams](../draft.md#one-combinator-algebra-for-trees-strings-and-streams).

### Q31. Does a friendlier string-pattern language belong in the language, and what regex flavor does it extend to?

OPEN. A URL-route-style syntax with named, typed captures composing through the existing
`int(.)`-style codec syntax is one candidate, with Swift's `Regex` builder and route-pattern DSLs
such as Express's `path-to-regexp` as the closest prior art. [The ordered arm list](../docs/guides/matching.md) already
commit any such language to ordered, PEG-style choice, which is compatible with PCRE/Perl-style
regex and not with POSIX leftmost-longest regex, so "extends to regular expressions" needs to
name which flavor. See
[One combinator algebra for trees, strings, and streams](../draft.md#one-combinator-algebra-for-trees-strings-and-streams).

### Q32. Does the dimension model subsume the effect layer?

The two-layer section says multiplicity lives either in a value or in evaluation, and
[the one-way shift](../docs/guides/cardinality.md#reify-the-one-crossing) narrows that to
effect multiplicity being born from streaming input and never from a value. The
[index-spec model](../docs/reference/operators/specs.md) says something that may be the same thing in different words: a value has an ordered
list of dimensions, and a spec says what happens to each.

Put them together and a `Stream` looks like a value with a dimension whose extent is not known
yet. The spec vocabulary already covers it without a second layer: keep and narrow are
streamable, since neither has to consume anything to know what it did, and collapse is not. That
distinction is written down in [the index-spec model](../docs/reference/operators/specs.md) and it is exactly the `Vec` and `Stream`
difference.

If that holds, there is one layer with a refinement rather than two layers, and the question
stops being which direction the shift runs and becomes whether anything shifts at all.

Three things would follow, and they are what makes this worth settling rather than leaving
implicit:

`map` stays primitive rather than becoming sugar for `[ .[] | f ]` later, since the thing it
would be sugar for never comes back.

`.[]` stays inert on a `Vec` permanently. Keeping every index of a known extent changes nothing,
and no future feature makes it change something.

The two-layer framing that opens this document becomes a description of a special case rather
than the organising idea.

Not proposed, because the two-layer section is load-bearing and this has not been worked through
against an actual streaming input. What prompted it: prototype 1 has no effect layer, and that
is not a departure from the design but what the design predicts for a program that reads one
whole value and hands one back. Whether the layer returns with streaming, or whether streaming
turns out to be a dimension, is the open part.

### Q33. Does a spread slot in a call give partial application?

Functions are unary and several arguments travel as one record, which makes a question available
that a positional language would have to answer with arity counting. If a call may leave a slot
open -- spelled `...` for now -- what comes back is a function expecting the fields that were
not supplied:

```
join {with: ", ", ...}
```

`join` takes `{over: Vec<Str>, with: Str}`, so supplying `with` leaves a function from
`{over: Vec<Str>}` to `Str`. The remaining parameter is **the complement of what was given**,
computed structurally rather than by position, so there is no question of which argument was
skipped and no need for placeholders in the other slots.

What makes this worth recording rather than dismissing is that it is not a feature bolted onto
the call syntax; it falls out of arguments already being a record. Partial application in a
positional language has to invent a convention for "this one, not that one". Here the convention
is subtraction on field names, which the type system already does.

Open, and roughly in dependency order:

- **Does it need first-class functions?** The language has none: `Sig` is a parameter and a
  result, and there is no function type in the type grammar. A partial application evaluates to a
  function, so it needs one to have a type. That is the real cost, and it is much larger than the
  syntax.
- **What does the residual type look like?** `{over: Vec<Str>} -> Str` needs an arrow in the type
  grammar, which is the same thing the previous point asks for.
- **Is `...` the right spelling?** It reads as "and the rest", which is right, but the token is
  unused and could go to a spread that *supplies* fields instead -- `{...defaults, n: 1}` --
  and those two meanings would collide.
- **Does supplying nothing mean anything?** `join {...}` would be `join` itself, which is either
  a harmless identity or a sign the spelling proves too much.
- **Does it interact with the dimension model at all?** A record literal does not distribute, so
  presumably not, but partial application inside `map` is exactly where it would be used most.

RULED option C (partial-application-system-design, 2026-09-07): array-like (positional prefix)
and struct-like (field subtraction) partial application are two input shapes hitting one
mechanism, application returning a function, once functions are first-class. The maintainer
added a requirement no option carried: partial application must always be syntactically
explicit, never inferred from arity or shape, so a residual function value is visibly distinct
from an array literal. Two things now need first-class functions besides this: `.`-rebinding
was ruled sugar over a one-parameter closure (`dot-rebinding-vs-lambda-design`, 2026-09-17),
and `dsv-partials-migration` is parked on a partial-application build row that does not exist
yet; `closures-first-class-functions-build` is the row this entry waits on.

### Q34. Do named types exist, and is a name an alias or an identity?

Deferred when record literals were settled, on the grounds that a literal synthesises its own
type and so forecloses nothing. What has since turned up is that the *constructor* for a named
type already works, which changes what the question costs without changing what it asks.

A constructor is a unary function from the structural record to the named type, and that shape
exists today:

```
fn User(c: {name: Str, age: Int}) -> {name: Str, age: Int} = c

User {name: "ada", age: 36}
```

`User {...}` parses and reaches the checker, which rejects it only because no function of that
name is defined. Type names and expression names already resolve by separate paths, so there is
no ambiguity to invent a rule for.

Free, then: the spelling, and the semantics, since a constructor being a unary function over a
record is the same decision already made for arguments generally.

Not free, in rough order of how much they decide:

- **Destructuring.** Does `.name` on a `User` see through the name, or does getting a field
  out need an explicit step? Nothing about this direction falls out, and it is what decides
  whether an identity is pleasant or a tax.
- **The declaration form.** Shipped: `type X = ...` declares a pure alias
  ([alias](../docs/reference/types/alias.md)), emitting identical bytes to the type written
  out on every backend, and `Type::from_name` now knows seven built-in names. What shipped
  answers the alias half only; an identity declaration for records is still absent.
- **One namespace or two.** The checker looks a call up in `sigs`, so a type declaration
  introducing a constructor would put type names and function names in one namespace. That is
  probably right and should be chosen rather than arrived at.
- **Alias or identity.** The question proper, and the only part the above does not touch. An
  alias abbreviates a type the tutorial's annotation shows is worth abbreviating; an identity
  makes two same-shaped records refuse to interchange, which is a different feature with a
  different justification.

Worth being explicit that cheapness is not an argument. What is recorded here is that the cost of
identity is lower than it looked, not that the language wants it.

[The enum decision](../docs/guides/enums.md) has since answered every bullet for
enums specifically: they are identities (exhaustiveness requires it), `enum` is the declaration
form, and variant constructors land in the value namespace with bare-until-ambiguous
resolution. Records are deliberately not carried along; the alias-or-identity question stays
open for them, and this entry now tracks only that half.

### Q35. What are stdout and stderr, and does a program write or return?

This document mentions `stdout` once and `stderr` never. For a language whose subject is
transforming data on a command line, that is not deferral, it is an oversight, and it is recorded
here rather than quietly fixed because the absence shaped things: every question about streams so
far has been about values coming *in*.

What exists is one answer by default. A program is an expression, its value is rendered by the
type-driven printer, and that is the whole of output. It has served: line-oriented output needed
no side effect, because a `Str` containing newlines already is line-oriented output.

What it does not answer:

- **Does a program write, or return?** Returning is what makes a program an expression and what
  keeps `map` reorderable, since a write is an effect and an effect is an ordering constraint.
  Writing is what a long-running filter over a stream has to do, because holding the output until
  the input ends is the thing streaming exists to avoid. These are not obviously reconcilable.
- **Is stderr in the language or under it?** Every backend refuses in its own words today, and the
  agreement harness deliberately checks only *that* they refuse. Making the message part of the
  language means seven backends must agree on it.
- **Does output have a type?** Input does, and it is checked. Output is whatever the body renders
  to, which means the printer is the only specification of the format.
- **Does a stream of outputs exist at all**, or does a program produce one value whose rendering
  happens to be long? jq answers the first; the design so far assumes the second without saying
  so.

Blocked on the same thing as [Q5](#q5-stream-lowering-strategy-across-the-three-backends): a
program that writes as it goes is a program with an effect layer.

The fused `jsonlines(f(inputs))` loop has since made write-as-it-goes real at the backend
level, and [the streams decision](../docs/adr/0001-stream-is-the-effect-layer-typed.md) gave the effect
layer a type -- so the blockage above is gone, and the question is sharpened rather than
answered. What was decided there about output is deliberately minimal: `jsonlines` is a sink,
legal only as the program's outermost expression, with no result type. That removes the old
placeholder (`jsonlines(...) : Str`, a type claiming the whole output exists as one value)
without deciding whether stdout is a value, an effect, or something a program returns into.

Ruled since, in three steps under `stdout-stderr-effect-model-design` and its two follow-ups
(2026-09-07). First, option A: opaque plumbing, a second single-instance sink beside
`jsonlines`, with no sequencing form (the [research](stdout-stderr-effect-model-research.md)
had leaned the other way, since a program is one expression and cannot write twice). Then,
because `pipe_through` must relay a subprocess's stdout and stderr in one run, a merged stream
whose items are tagged by origin; and finally the mechanism for that tag, one fixed prelude
enum rather than any new `Alt<A, B>` machinery. The shipped shape is `PipeLine`
(`Stdout{text}` or `Stderr{text}`) and
[`pipe_through`](../docs/reference/builtins/pipe_through.md), built on the Rust backend. So
stderr is in the language, as a variant. What remains open from the list above is the
splitting of stdin and stdout, which every round flagged and none answered; it has its own
row.

### Q36. Does a real module system need imports, multiple files, and enforced privacy?

One file exists: `prelude.toy`, always merged in whole, `pub` picking which of its definitions a
program receives. What it does not have: a way to name what a program wants rather than receiving
all of it; a way for a program's own file to export something another file imports; and more
than one file to import from at all.

Privacy is no longer on that list. The forcing event this entry predicted, a second prelude
function needing an internal helper, happened: `join` and `join_lines` share the private
`join_parts`, and `file-visibility-tracking-build` made a non-`pub` definition callable only
from its own file, enforced by origin at each call site
([the prelude page](../docs/reference/prelude/index.md)). The prelude now holds two
functions, three enums, and a trait with one impl, so the "one function out of seven
backends" framing is gone too. On syntax: `@(path)` module routing is built (2026-09-19,
`module-routing-semantics-build-2`, on the four rulings recorded on the archived
`module-routing-semantics-build` row): a routed file is loaded relative to the file that
names it, merged prelude-style with its own `Origin`, and its `handle` is called with `.`
checked strictly against the declared parameter; see
[modules](../docs/reference/syntax/modules.md). That answers "more than one file" for the
routing shape; a named import list and a program exporting for another file are still the
open part.

### Q37. How do floats print, and what are NaN and Infinity in a JSON-shaped value model?

The representation is [decided](../docs/adr/0007-float-is-javascripts-double.md): IEEE 754 binary64,
JavaScript's number. The three observable pieces below were each open when this was written
and are each settled and built now (ruling kantord/toylang#145, build kantord/toylang#149,
2026-09-04 to 2026-09-06; [Float](../docs/reference/types/float.md)): printing is ECMA-262
`Number::toString` on all seven backends, verified byte for byte against Node over a
fuzz run, with a hand-written formatter in the native runtime because libc has none; `NaN`
and `Infinity` are admitted as values and print by name, so a top-level result holding one is
not JSON; and `Float` division by zero returns `Infinity`, so division's failure behavior
does depend on the operand type. The jq backend's nested-float
divergence (a `Float` inside a `Vec` or record printed in jq's own notation, and the
non-finite values as the largest double or `null`) was closed the same day the docs harness
found its wider half, 2026-09-18, by rendering any Float-bearing structure as text. There is still no corpus case for `Float`
(`float-corpus-cases`). The original framing follows.

- **Printing.** Every backend must render the same double to the same text, and their defaults
  do not agree on shortest-roundtrip versus fixed formatting, or on `1e21`-style switchover
  points. The printer is currently the only specification of output format
  ([Q35](#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return)), which makes
  this a per-backend conformance rule to be stated by hand, the same lesson as
  [backends can agree and still be wrong](../research-log/backends-can-agree-and-still-be-wrong.md).
- **NaN and Infinity.** IEEE produces both; JSON can spell neither. A language whose values
  are JSON-shaped either forbids them (a check on every producing operation), maps them to
  something at the boundary (jq-style lossiness, the kind this design usually refuses), or
  admits values its own output cannot carry.
- **Division by zero.** The Int rule says a zero divisor is the only way arithmetic fails.
  IEEE says `1.0 / 0.0` is `Infinity`, no failure at all. Keeping both means division's
  behavior depends on its operand type; unifying means overriding one standard or the other.

None of this blocked anything else, so it waited for `Float` to be forced by a real program,
which the benchmark suite's mandelbrot did.

### Q38. Are composites ordered at all?

The equality decision ([comparison](../docs/reference/operators/comparison.md)) made equality
on a record or enum structural and stopped at a `Vec`, but deliberately left ordering alone.
`<` on a record typechecks today and the backends disagree on it: three refuse to compile it,
two fail at runtime, and two answer -- jq by its own document order, JS by comparing the string
`[object Object]` against itself. Whether composites should be ordered at all is a question
nobody has been asked, so the disagreement stands until someone is.

### Q39. Is a timestamp type worth a third numeric type?

`Int`'s named cost is that millisecond timestamps (1.8e12, past its 2.1e9 ceiling) are
rejected at the input validator, with `Int64` as the fix that covers them ([int64](../docs/reference/types/int64.md)).
`Int64` is for identifiers and timestamps -- values that are *carried*, not computed -- but it is
also a real 64-bit integer whose arithmetic wraps past 2^63, which a timestamp wants nothing
to do with. A dedicated timestamp type is a separate question again, and possibly a better
answer than an integer either way, but nothing has forced it yet.

### Q40. Is a `Field<K>` lens trait part of the design?

The draft sketched `.foo` desugaring to a `Field<K>` trait -- `get`, `path`, `set` -- so a path
expression is simultaneously a getter, a setter, and a path witness, making update-in-place,
deletion, and path enumeration one syntax. None of that is built:the spelling is not documented
behavior. [Q10](#q10-is-uniqueness-analysis-in-scope-for-deciding-when-a-lens-materializes)
asks when a lens materializes,and [Q12](#q12-on-a-type-mismatch-does-field-access-error-yield-null-or-something-third)
records the value/absence/error distinction, but neither carries the trait itself. Its law --
what `set` promises about the path, whether `path` witnesses updates, deletions, or both,
and how the receiver changes the implementation (indexable versus iterable) -- is unwritten.
Recorded so the sketch survives the draft's deletion without being built.

### Q41. Is narrowing a record to a subset of its fields an operation?

Punning -- `{name}` for `{name: .name}` -- was refused for a reason that leaned on this
question: narrowing a record to some of its fields is arguably its own operation, the way
`select` narrows a dimension, and the language has not decided it, so sugar that quietly
implements one answer makes the question harder to ask (see
[Records](../docs/reference/types/record.md)). The refusal leaves the question open rather
than answering it. What is unsettled is whether narrowing is a first-class operation at all,
and if so what its spelling is -- a record literal already builds one, and projection already
reads fields, but neither names the act of taking a subset. Recorded so the reason the
shorthand was refused stays findable now that the draft section carrying it is gone.

### Q42. Is a runtime field-names accessor part of the design?

A record's declared field order is real data -- it drives printing and the native/Go columnar
layouts (see [Records](../docs/reference/types/record.md)) -- and is meant to become a
runtime-queryable accessor for serialization and friends. Built as
[`fields`](../docs/reference/builtins/fields.md) (`field-names-accessor`, from
kantord/toylang#63), on every backend, reflecting the checked declaration order.
