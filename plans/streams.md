# Typed streams: making the checker stop lying

Implements draft.md's "DECIDED: Stream is the effect layer, typed" and ADR 0001. The design is
settled; what follows is build order. The point of the whole feature, kept in view throughout:
today the checker types `inputs` as `Vec<T>` and `jsonlines(...)` as `Str`, and whether a
program streams is decided by `tir::recognize_fusion`'s structural guess -- a program one shape
away from the pattern silently materializes all of stdin. At the end, the types say what
streams, the eager spelling is a greppable `collect`, and a stream-typed program that fails to
stream is a compiler bug rather than a silent behavior.

The implementation template is `Lines`: the checker already has a monomorphic second-class
stream type -- unprintable, banned from records and Vecs, unspellable in signatures, single-use,
with `collect` as its one exit. Nearly every step below generalizes something `Lines` already
does rather than inventing machinery. The genuinely new pieces are two: `Stream` becoming
spellable (the one thing `Lines` deliberately withheld), and per-binding linearity (the
`lines_used` flag is "referenced at all, twice" on one global; a stream-typed function
parameter needs "consumed exactly once" per binding).

## Step 1: generalize Lines to Type::Stream, invisibly

`Type::Lines` becomes `Type::Stream(Box<Type>)`; `lines` is born `Stream<Str>`; `collect`'s
signature generalizes from `Lines -> Vec<Str>` to `Stream<T> -> Vec<T>` (checked the way the
other polymorphic builtins are, argument synthesised first). The containment bans (record
field, Vec element, printed result) and the mutual-exclusion and single-use checks carry over
untouched. No surface syntax changes, no corpus case changes, every existing test stays green:
this step is a refactor whose only observable is that error messages may now say "a stream"
where they said `lines`.

## Step 2: make Stream spellable, and make it linear

The type grammar accepts `Stream<T>` in function signatures. That un-does the load-bearing
trick the `Lines` design recorded ("a return annotation can never spell Lines"), so what that
trick guaranteed for free must now be checked for real:

- A `Stream`-typed parameter is consumed exactly once in the function body. Zero uses is an
  error (linear, not affine -- the decision's reversibility argument), two is the Python-
  generator silent-empty-iterator mistake the single-use rule exists to prevent.
- The containment bans apply to signature-spelled streams identically: `Vec<Stream<T>>` and a
  `Stream`-typed record field are rejected in the type grammar itself, not just at value
  construction sites.

Rejection tests (step-style, insta): unused stream parameter, twice-consumed stream, `Stream`
under a value constructor in an annotation, `Stream<Stream<T>>`. The corpus cannot see any of
this; these snapshots are its only witness, the same blindness every checker feature hits.

## Step 3: mappers over streams

`map`, `select`, and projection accept a `Stream` subject, yielding a `Stream` of the mapped
element type -- the same subject-context mechanism that types them over `Vec` today, with the
element drawn from `Stream`'s parameter instead. With this, `fn f(s: Stream<A>) -> Stream<B>`
whose body is a pipeline over `s` checks end to end. A corpus case exercises a user-written
stream-signature function on all seven backends (eagerly lowered is fine at this step; output
is what the corpus can check).

## Step 4: retype inputs, and jsonlines becomes a sink

`inputs` is born `Stream<T>`, element type inferred from use exactly as before. This is the
breaking step, and the three affected corpus cases migrate to the honest spellings the
decision records: the two mapper-shaped cases get `Stream` signatures, and the eager one
becomes `total(collect(inputs))`. `extent` stays `Vec`-only -- its no-fold promise holds, and
there are no stream reducers in this plan.

`jsonlines` simultaneously stops having a result type: legal only as the program's outermost
expression, accepting `Vec<T>` or `Stream<T>`. The placeholder `Str` typing dies, per the
decision ("a type claiming the whole output exists as one value"). Rejection tests: a nested
`jsonlines`, a program whose result is a bare unconsumed `Stream` (generalizing the "contains
`lines`, nothing to print" error), `collect` of a `Vec`.

## Step 5: fusion reads types instead of guessing shapes

`recognize_fusion`'s structural match retires. The backends compile a stream-typed pipeline
ending in `jsonlines` as the fused read/transform/write loop they already know how to emit,
driven by the types; a pipeline ending in `collect` materializes at the `collect` and stays
eager past it. `lib.rs`'s `streams_inputs` gate becomes a type question. The invariant this
step must land: for a stream-typed live program, the eager fallback path is unreachable --
if the types say stream and no loop is emitted, that is a bug, not a behavior.

`tests/streaming.rs` gains a liveness probe for a program the old recognizer would have
rejected (a user-written `Stream -> Stream` function in the pipeline), which is the probe that
proves the guess actually retired. The payoff corpus case pairs the features shipped this
week: NDJSON of mixed messages, `inputs : Stream<SomeEnum>`, one exhaustive match inside
`map`, fused, on all seven backends.

## What this plan does not include

Stream reducers (a fold over a stream is real design work on `extent`'s no-fold promise);
the pattern combinators of ADR 0008 (`Seq`/`Alt`/plus -- they wait for a source that produces
a non-star pattern); Q35 (what stdout is -- the sink rule deliberately avoids answering);
reflect (`.[]` as value-to-effect, still leaning "does not exist"); fan-out and concurrency
(named non-goals of the pull decision). Each has its trigger recorded in the draft.
