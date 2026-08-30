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
from then on. Where no such record exists yet -- Q20 is the only one today -- the entry keeps
its detail, because collapsing it would delete the only copy.

| # | Question | Status |
|---|---|---|
| [Q1](#q1-streams-first-class-values-or-evaluation-level-multiplicity) | Streams: first-class values, or evaluation-level multiplicity? | SETTLED, evaluation-level and typed: `Stream<T>` is the effect layer's type, not a value type |
| [Q2](#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit) | Binary operators over two multi-valued expressions: cartesian, zip, or explicit? | OPEN |
| [Q3](#q3-what-symbol-replaces--for-the-record-forming-update) | What symbol replaces `=` for the record-forming update? | LEANING, blocked on Q2 |
| [Q4](#q4-can-the-type-express-ordering-over-heterogeneous-streams) | Can the type express ordering over heterogeneous streams? | OPEN, but the shape is decided (ADR 0008: Kleene patterns in effect position); enums now supply tagged alternation, leaving the matcher surface and spelling |
| [Q5](#q5-stream-lowering-strategy-across-the-three-backends) | Stream-lowering strategy across the three backends | OPEN in general; all seven backends stream the fused pipeline shape, so only lowering beyond that shape remains |
| [Q6](#q6-does-a-reconciler-belong-in-the-language-or-a-library) | Does a reconciler belong in the language or a library? | OPEN |
| [Q7](#q7-does--promise-depth-first-order-or-only-the-set-of-nodes) | Does `..` promise depth-first order, or only the set of nodes? | OPEN |
| [Q8](#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization) | Is vectorizability visible in the type system, or a silent optimization? | OPEN |
| [Q9](#q9-are-vectors-multidimensional-with--as-projection) | Are vectors multidimensional, with `[]` as projection? | OPEN, may merge with Q2 |
| [Q10](#q10-is-uniqueness-analysis-in-scope-for-deciding-when-a-lens-materializes) | Is uniqueness analysis in scope, for deciding when a lens materializes? | LEANING yes, compiler-internal: see the privileged-references sketch |
| [Q11](#q11-how-does-the-querytransformation-split-manifest-in-the-type-system) | How does the query/transformation split manifest in the type system? | SETTLED |
| [Q12](#q12-on-a-type-mismatch-does-field-access-error-yield-null-or-something-third) | On a type mismatch, does field access error, yield null, or something third? | SETTLED |
| [Q13](#q13-does-the-layer-shift-run-only-one-way-with-no-value-to-effect-operator) | Does the layer shift run only one way, with no value-to-effect operator? | LEANING yes, and now load-bearing: born-at-sources, dies-at-exits is the `Stream<T>` typing rule |
| [Q14](#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy) | Does `select` return a masked view, a selection vector, or a copy? | OPEN |
| [Q15](#q15-backend-llvm-via-inkwell-cranelift-or-both) | Backend: LLVM via inkwell, Cranelift, or both? | SETTLED, LLVM via inkwell, built and running |
| [Q16](#q16-string-representation-given-wtf-16-on-the-js-target) | String representation, given WTF-16 on the JS target | OPEN, decides the string API permanently |
| [Q17](#q17-is-there-a-dense-tensor-type-constructed-explicitly) | Is there a dense tensor type, constructed explicitly? | LEANING yes |
| [Q18](#q18-does--on-a-rank-2-tensor-yield-rows-or-scalars) | Does `.[]` on a rank-2 tensor yield rows or scalars? | LEANING rows |
| [Q19](#q19-how-are-nulls-carried-in-a-dense-typed-buffer) | How are nulls carried in a dense typed buffer? | LEANING, Arrow validity bitmask |
| [Q20](#q20-how-are-blocking-operators-sort-group_by-joins-classified) | How are blocking operators (`sort`, `group_by`, joins) classified? | SETTLED, a trait with no lawful stream instance |
| [Q21](#q21-what-guarantees-batch-size-is-unobservable-over-a-batched-stream) | What guarantees batch size is unobservable over a batched stream? | LEANING, the trait law that ops commute with reification |
| [Q22](#q22-are-dense-and-masked-vectors-distinguishable-in-the-type) | Are dense and masked vectors distinguishable in the type? | OPEN, Q14 from the other side |
| [Q23](#q23-what-primitive-set-is-the-standard-library-defined-over) | What primitive set is the standard library defined over? | LEANING, the parallel basis |
| [Q24](#q24-are-compile-time-macros-a-first-class-concept) | Are compile-time macros a first-class concept? | OPEN, not yet evaluated |
| [Q25](#q25-does-the-language-have-union-types) | Does the language have union types? | PARTLY SETTLED: closed nominal sums exist (enums); anonymous structural unions remain an absence |
| [Q26](#q26-is-jsxs-children-slot-a-closed-per-site-union-or-an-open-one) | Is JSX's children slot a closed per-site union, or an open one? | OPEN, deliberately deferred to last |
| [Q27](#q27-does-pattern-matching-need-a-separate-matcher-type-distinct-from-result) | Does pattern matching need a separate `Matcher` type, distinct from `Result`? | SETTLED |
| [Q28](#q28-does-deep-matching-need-cross-match-unification-of-logic-variables) | Does deep matching need cross-match unification of logic variables? | OPEN |
| [Q29](#q29-what-is-the-default-discriminant-convention-for-a-derived-enum-codec) | What is the default discriminant convention for a derived enum codec? | SUPERSEDED: the enum decision made the single-key wrapper the value itself, not a codec default |
| [Q30](#q30-do-the-base-functor-generics-double-as-parser-combinators-across-trees-strings-and-streams) | Do the base-functor generics double as parser combinators, across trees, strings, and streams? | LEANING yes, implementation split still open |
| [Q31](#q31-does-a-friendlier-string-pattern-language-belong-in-the-language-and-what-regex-flavor-does-it-extend-to) | Does a friendlier string-pattern language belong in the language, and what regex flavor does it extend to? | OPEN |
| [Q32](#q32-does-the-dimension-model-subsume-the-effect-layer) | Does the dimension model subsume the effect layer? | OPEN, and it may dissolve Q13 rather than answer it |
| [Q33](#q33-does-a-spread-slot-in-a-call-give-partial-application) | Does a spread slot in a call give partial application? | OPEN, and only expressible because arguments are a record |
| [Q34](#q34-do-named-types-exist-and-is-a-name-an-alias-or-an-identity) | Do named types exist, and is a name an alias or an identity? | OPEN for records; enums decided identity for themselves, and enum declarations are the first declaration form |
| [Q35](#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return) | What are stdout and stderr, and does a program write or return? | OPEN; `jsonlines` is now a top-level-only sink with no result type, which removes a placeholder answer without deciding the question |
| [Q36](#q36-does-a-real-module-system-need-imports-multiple-files-and-enforced-privacy) | Does a real module system need imports, multiple files, and enforced privacy? | OPEN, one always-on prelude file exists; nothing beyond it does |
| [Q37](#q37-how-do-floats-print-and-what-are-nan-and-infinity-in-a-json-shaped-value-model) | How do floats print, and what are NaN and Infinity in a JSON-shaped value model? | RULED (gh:145): admit NaN/Infinity, division by zero returns Infinity (matches IEEE). Printing format is still open per-backend conformance work; tracked at board row `float-build` |

[Multidimensional vectors](#q9-are-vectors-multidimensional-with--as-projection) is the one
question still capable of changing [the two-layer
section](../draft.md#the-core-idea-two-layers), now that
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
is still open.

Composite equality is settled without touching it. `==` on a record or an enum compares
structurally, and is refused outright when the type carries a Vec anywhere inside it, so a
`Vec`-typed record field never quietly acquires whole-value semantics
([the equality decision](../draft.md#decided-equality-on-a-composite-is-structural-and-stops-at-a-vec)).

### Q3. What symbol replaces `=` for the record-forming update?

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

An exploration after [the streams decision](../draft.md#decided-stream-is-the-effect-layer-typed)
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
`jsonlines(f(inputs))` loop showed is also false: all seven backends now stream that pipeline
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

### Q8. Is vectorizability visible in the type system, or a silent optimization?

Reporting it
means a second effect alongside cardinality, and a visible fast-path/slow-path distinction in
signatures. Hiding it makes performance unpredictable in exactly the way this design is
trying to avoid. Note the two effects are orthogonal: `select` changes cardinality and
vectorizes fine as a mask, while `first` changes cardinality the same way and cannot
vectorize at all.

### Q9. Are vectors multidimensional, with `[]` as projection?

See the TODO and response in the
cardinality section. Unifies indexing with iteration, but disturbs the claim that there are
exactly two layer shifters, and per-dimension cardinality only describes rectangular data
while JSON is ragged.

### Q10. Is uniqueness analysis in scope, for deciding when a lens materializes?

Deciding when a projection lens can materialize instead
 of staying a view requires knowing no other reference to the source survives. That is
 linearity or uniqueness typing, the machinery deliberately avoided in [the ordering question](#q4-can-the-type-express-ordering-over-heterogeneous-streams).

### Q11. How does the query/transformation split manifest in the type system?

SETTLED: it does not need to. `map` and `select` are the same operation with the multiplicity
stored in different places, so the split never reaches the type system; see [the two-layer
section](../draft.md#the-core-idea-two-layers).

### Q12. On a type mismatch, does field access error, yield null, or something third?

SETTLED: something third. Field access desugars to a lens with three distinguishable outcomes
-- a value, a specific absence, and a specific error; see [the field-access
section](../draft.md#field-access-is-a-lens).

### Q13. Does the layer shift run only one way, with no value-to-effect operator?

If effect multiplicity is born only from
 streaming sources and dies only into values through `[...]`, then no value-to-effect
 operator is needed, because degrading a `Vec` forgets its extent and buys nothing. LEANING
 toward yes. This decides [the streams question](#q1-streams-first-class-values-or-evaluation-level-multiplicity) with it, since the only thing that would break it is a value
 with genuinely unknown extent, which is what a first-class stream value would be. The streams
 question is now settled the compatible way, and this lifecycle -- born at `inputs`/`lines`,
 dead at `collect` or a sink -- became the `Stream<T>` typing rule, so reversing this lean now
 means amending that decision too.

### Q14. Does `select` return a masked view, a selection vector, or a copy?

See the section on
 whether a value-layer `select` copies. A bitmask breaks `Vec`'s constant-time indexing
 promise, a selection vector keeps it and pays memory per survivor, and either view pins its
 whole source buffer alive.

### Q15. Backend: LLVM via inkwell, Cranelift, or both?

SETTLED: LLVM via inkwell, built and running. Recorded in
[ADR 0005](../docs/adr/0005-llvm-via-inkwell-for-the-native-backend.md).

### Q16. String representation, given WTF-16 on the JS target

The three
 options are WTF-16 everywhere, UTF-8 everywhere with the JavaScript-shaped API emulated, or
 designing the difference away by never exposing code-unit indexing or length. Only the third
 is cheap on both sides. It has to be decided early because it constrains the string API
 permanently.

### Q17. Is there a dense tensor type, constructed explicitly?

`@f32` as a narrowing constructor
 that hard-fails rather than an inference, with `reshape` attaching shape. It is also the
 second number type, a deliberate lossy exit from the `f64` commitment.

### Q18. Does `.[]` on a rank-2 tensor yield rows or scalars?

NumPy and APL both yield rows,
 which makes `map` rank-polymorphic and gives row sums as `map(fold(add; 0))` with no new
 syntax. Then rank-1 yields scalars and full linearization needs a separate flattening view.

### Q19. How are nulls carried in a dense typed buffer?

JSON has null and an `f32` buffer does
 not. NaN as a sentinel collides with genuine NaN. Arrow's separate validity bitmask solves
 it and brings zero-copy interop with Polars, DuckDB and pandas.

### Q20. How are blocking operators (`sort`, `group_by`, joins) classified?

`sort`, `group_by` and joins are one value in and
 one value out, so the per-element cardinality mapping does not describe them. They need the
 whole input before producing anything and are parallelizable by other means. The
 kernel-admissibility result covers elementwise filters only, and this is the gap it leaves.

### Q21. What guarantees batch size is unobservable over a batched stream?

Argued in [the admissible input set, and where batching comes from](../draft.md#the-admissible-input-set-and-where-batching-comes-from) rather than
here, since it arrived with that material. The leaning is the trait law that operations commute
with reification, which is what makes a batch boundary invisible to a program.

### Q22. Are dense and masked vectors distinguishable in the type?

Argued in [the admissible input set, and where batching comes from](../draft.md#the-admissible-input-set-and-where-batching-comes-from), where it
appears as the observation that a masked view and a dense buffer have different launch
preconditions. The same question as [what select returns](#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy), approached from
the layout side rather than the operator side.

### Q23. What primitive set is the standard library defined over?

Argued in [the primitive set cannot be fold and recursion](../draft.md#the-primitive-set-cannot-be-fold-and-recursion). The leaning is the
parallel basis, with `fold` and general recursion available but not the thing everything else
is defined over.
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
piece of machinery rather than two.

Partly settled by [the enum decision](../draft.md#decided-enums-nominal-and-json-native): closed nominal
sums now exist, and they serve both this question's motivating case (heterogeneous data) and the
ordering question's `Alt` (a stream of several message kinds is `Stream<SomeEnum>`). What
remains absent is the anonymous structural union, `Str | Int` with no declaration -- a
different feature with a different justification, still an absence rather than a decision.

### Q26. Is JSX's children slot a closed per-site union, or an open one?

Sketch: a creator function taking a `Record` of strictly-typed attrs (this is just `Field`
in the existing sense, no new machinery) plus a `Dimension` of children. The children slot needs
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
capital name (kantord/toylang#47). See [the enum decision's construction and naming
section](../draft.md#construction-and-naming) and
[guides/matching.md](../docs/guides/matching.md).

### Q28. Does deep matching need cross-match unification of logic variables?

OPEN. `..` composed with a matcher already finds a shape anywhere in a tree without naming its
path, and `as` already binds one submatch to a name for reuse within the same arm. Neither needs
unification. What would: finding a node `A` and a separate node `B` elsewhere such that `B`
refers to `A`, which is full Prolog-style unification with backtracking over bindings, not a
bigger version of `as`. See [Pattern matching is decoding](../draft.md#pattern-matching-is-decoding).

### Q29. What is the default discriminant convention for a derived enum codec?

SUPERSEDED: there is no derived codec picking a representation, because the representation
*is* the value. [ADR 0009](../docs/adr/0009-enums-are-json-native-single-key-wrappers.md)
records the decision, and why the tag-field and shape-matched alternatives lost.

### Q30. Do the base-functor generics double as parser combinators, across trees, strings, and streams?

LEANING yes. `Seq`, `Alt`, `Star`, and `Opt` are already in the document as [the regex-over-types algebra](#q4-can-the-type-express-ordering-over-heterogeneous-streams)
and as the shape [Pattern matching is decoding](../draft.md#pattern-matching-is-decoding) builds `Matcher<T>` from; naming them as parser
combinators only makes the precedent explicit (Hutton and Meijer; Wadler; parsing with
derivatives). OPEN: whether this is one trait with implementations that differ by receiver (a
parsed tree needs no backtracking, a string needs an actual parsing engine), the same shape as
[`Field<K>`](../draft.md#field-access-is-a-lens), and if so what law the implementations have to share. See
[One combinator algebra for trees, strings, and streams](../draft.md#one-combinator-algebra-for-trees-strings-and-streams).

### Q31. Does a friendlier string-pattern language belong in the language, and what regex flavor does it extend to?

OPEN. A URL-route-style syntax with named, typed captures composing through the existing
`int(.)`-style codec syntax is one candidate, with Swift's `Regex` builder and route-pattern DSLs
such as Express's `path-to-regexp` as the closest prior art. [The arm-list's `//` semantics](../draft.md#pattern-matching-is-decoding) already
commit any such language to ordered, PEG-style choice, which is compatible with PCRE/Perl-style
regex and not with POSIX leftmost-longest regex, so "extends to regular expressions" needs to
name which flavor. See
[One combinator algebra for trees, strings, and streams](../draft.md#one-combinator-algebra-for-trees-strings-and-streams).

### Q32. Does the dimension model subsume the effect layer?

The two-layer section says multiplicity lives either in a value or in evaluation, and
[the one-way shift](../draft.md#proposal-the-layer-shift-only-runs-one-way) narrows that to
effect multiplicity being born from streaming input and never from a value. The dimension
proposal says something that may be the same thing in different words: a value has an ordered
list of dimensions, and a spec says what happens to each.

Put them together and a `Stream` looks like a value with a dimension whose extent is not known
yet. The spec vocabulary already covers it without a second layer: keep and narrow are
streamable, since neither has to consume anything to know what it did, and collapse is not. That
distinction is written down in the dimension proposal and it is exactly the `Vec` and `Stream`
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

Blocked on first-class functions, which nothing else currently needs.

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
- **The declaration form.** `Type::from_name` knows three names and there is no `type X = ...`.
- **One namespace or two.** The checker looks a call up in `sigs`, so a type declaration
  introducing a constructor would put type names and function names in one namespace. That is
  probably right and should be chosen rather than arrived at.
- **Alias or identity.** The question proper, and the only part the above does not touch. An
  alias abbreviates a type the tutorial's annotation shows is worth abbreviating; an identity
  makes two same-shaped records refuse to interchange, which is a different feature with a
  different justification.

Worth being explicit that cheapness is not an argument. What is recorded here is that the cost of
identity is lower than it looked, not that the language wants it.

[The enum decision](../draft.md#decided-enums-nominal-and-json-native) has since answered every bullet for
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
  language means six backends must agree on it.
- **Does output have a type?** Input does, and it is checked. Output is whatever the body renders
  to, which means the printer is the only specification of the format.
- **Does a stream of outputs exist at all**, or does a program produce one value whose rendering
  happens to be long? jq answers the first; the design so far assumes the second without saying
  so.

Blocked on the same thing as [Q5](#q5-stream-lowering-strategy-across-the-three-backends): a
program that writes as it goes is a program with an effect layer.

The fused `jsonlines(f(inputs))` loop has since made write-as-it-goes real at the backend
level, and [the streams decision](../draft.md#decided-stream-is-the-effect-layer-typed) gave the effect
layer a type -- so the blockage above is gone, and the question is sharpened rather than
answered. What was decided there about output is deliberately minimal: `jsonlines` is a sink,
legal only as the program's outermost expression, with no result type. That removes the old
placeholder (`jsonlines(...) : Str`, a type claiming the whole output exists as one value)
without deciding whether stdout is a value, an effect, or something a program returns into.
Everything in the bullet list above remains open.

### Q36. Does a real module system need imports, multiple files, and enforced privacy?

One file exists: `prelude.toy`, always merged in whole, `pub` picking which of its definitions a
program receives. What it does not have: a way to name what a program wants rather than receiving
all of it; a way for a program's own file to export something another file imports; more than one
file to import from at all; and enforced privacy, since a non-`pub` prelude definition today is
not "private to the prelude," it is simply never compiled, which forecloses a `pub` function
calling a private helper.

None of these were needed to get one function (`unlines`) out of six backends' worth of hand-
written codegen, and building them speculatively risks shaping them around a prelude that has
exactly one function in it. What would force an answer: a second prelude function needing an
internal helper, at which point the non-`pub`-is-simply-absent rule stops being free.

### Q37. How do floats print, and what are NaN and Infinity in a JSON-shaped value model?

The representation is [decided](../draft.md#decided-float-is-javascripts-double): IEEE 754 binary64,
JavaScript's number. Everything observable about it is not, and each piece has to survive the
agreement harness, which checks bytes.

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

None of this blocks anything else, so it waits for `Float` to be forced by a real program the
way `inputs` and `jsonlines` were.
