# Batch type design: what a `Stream<Batch<T>>` type is for, and who may create one

Research spike for [Q21](questions.md#q21-what-guarantees-batch-size-is-unobservable-over-a-batched-stream),
requested after round 4 of the `offload-boundary-design` grill. Round 4 ratified a bare
compiler gate (a declared associative combinerat every batch boundary), and the maintainer's
answer asked for a richer effect type instead: a distinct `Stream<Batch<T>>` vs plain
`Stream<T>`, an explicit `[Stream<T>, N] -> Stream<Batch<T, N>>` batching function, and an
answer to what initiates batching (explicit marker, implicit at certain constructs, or
forced for certain operations). This writes up what that type would concretely mean, grounded
in what the language has today and in how three chunked-stream systems (Spark partitions, Flink
windows, Beam bundles) handle batch boundaries. Findings feed a re-ask of Q21
(docs/.grill/offload-boundary-design-round-5.round.yaml).

## What the language actually does today

Everything below is verified against the current compiler (`toylang run FILE`) unless
marked as a proposed shape.

`Stream<T>` is the effect-layer multiplicity ([ADR
0001](../docs/adr/0001-stream-is-the-effect-layer-typed.md)): an expression yields its
entries one at a time as evaluation proceeds, not a value. A stream is born at a source
(`stdin`, `range`) and dies at `collect` or the `jsonlines` sink. In between only `map`,
`select`, and projection accept a `Stream` subject. Streams are consumed exactly once, never
sitting in a record, a `Vec`, or another `Stream`, and cannot be printed. The checker
enforces each rule at compile time.

Batching does not exist yet. `stdin` is `Stream<Str>` today, and `range` is
`Stream<Int>`. The draft's `Stream<Vec<T>>` reader typing ([the admissible input set, and
where batching comes from](../draft.md#the-admissible-input-set-and-where-batching-comes-from)) is
design prose, not built: nothing in `src/` knows a batch, and the type grammar reserves
exactly two constructors (`Vec`, `Stream`). An operation over a stream that needs the whole
input is refused at compile time today:

```toylang
sum(range(6)
```

```error
`sum` needs a Vec of Int or Int64, found Stream<Int> (at byte 4)
```

```toylang
sort(range(6)
```

```error
`sort` needs a Vec, found Stream<Int> (at byte 5)
```

`first`, `length`, and `sum` refuse streams the same way. The one thing that makes a
stream whole is `collect` (a `Stream<T> -> Vec<T>` builtin). The draft's law (
`op(f) . reify == reify . op(f)`) is therefore still a paper law: nothing in the compiler
depends on it yet, because the only stream consumers are elementwise and the only thing that
stops streaming is `collect`. The design pressure behind batching is the GPU offload
story the draft reasons about -- kernels want chunks -- not something the current six
backends exercise.

## The problem `Stream<Vec<T>>` has, that `Batch<T>` is supposed to fix

The draft gives stdin the type `Stream<Vec<T>>`: "batched by the reader, the batching is
in the type." But a `Vec` is a first-class value: it has `length`, indexing, equality,
`sort`, printing. Make batches `Vec`s and a program can ask each batch how long it is, or
print a batch, or sort one. Batch size becomes observable, and the naive
average-of-averages works:

```toylang
# proposed shape, does not compile today
fn naive_mean(s: Stream<Vec<Int>>) -> Float =
    sum(batch_means(s)) / length(batch_means(s)
```

Round 4's gate (a declared associative combinerat every batch boundary) rejects
this particular program. But the gate is per operation: every future operation that accepts
a `Vec` is a new hole. A `batch_length(s: Stream<Vec<T>>) -> Stream<Int>` would be
legal under round 4's rule, and it would observe batch sizes directly. The maintainer's
ask is to move the enforcement from the operations to the type: make the boundary itself
unnameable, so no operation can observe it.



**`Batch<T>` is a type with no operations.** It exists only as the element of a
`Stream`, second-class in the same way `Stream` itself is: never inside a `Vec`, a record, another
`Stream`, or an enum payload; not a value, cannot be printed, consumed exactly once. It
has no `length`, no indexing, no equality, no `collect` -- because any of those would make
the boundary observable. Elementwise ops (`map`, `select`, projection) work over
`Stream<Batch<T>>` transparently: they are per-element, so they never see a batch, and the
boundary survives them unchanged. That is the draft's commuting law now enforced by the
type: you cannot write an operation that sees a batch, because there is no way to name a
batch as a value.



What the richer type buys, concretely:

- **Reductions over streams become expressible.** Today `sum(range(6))` is refused.
With `Stream<Batch<T>>`, `sum` over a batched stream is a two-level reduction (within
each batch, then across batches), sound exactly when `+` is associative -- which round 4
already ratified requiring. The gate stops being a rule bolted onto each operation and becomes
a property of the type: a batch boundary is exactly the place a two-level reduction happens,,
and the type records it.


- **The naive mean becomes unwritable, not merely rejected.** With `Batch<T>`
opaque, you cannot `length` a batch, so the naive mean cannot be typed at all. The honest
mean (sum over the whole, count over the whole, both associative reductions) is
expressible, and batch boundaries are invisible to it. Enforcement by construction, not
by a list of blessed operations.


- **Cost differences stay visible in the signature.** The draft's "cost still differs
where the law holds, and that is fine": `map` over a `Vec` is one launch; over a batched
stream it is one per batch. With `Stream<Batch<T>>` distinct from `Stream<T>`, the type
says which transport you asked for, so the cost difference is in the signature rather than
hidden in dispatch. This is the same move as the effect-layer decision (Stream is the
effect layer) and the string-representation decision (UTF-8 and UTF-16 are both allowed
precisely because no program can observe which it got).



## What `N` in `Batch<T, N>` can and cannot mean

The maintainer's sketch wrote `[Stream<T>, N] -> Stream<Batch<T, N>>`. `N` in the
result type has two problems. First, it needs a type-level natural (const generics): the
type grammar today has exactly two constructors and no kind of type-level number; adding
one is a second feature as large as adding `Batch` itself. Second, and worse: an exact
`N` would make batch size *known* to the program, not merely unobservable. With
equal-size batches, the naive average-of-averages becomes correct (mean of batch means ==
mean when every batch has the same length, up to the last partial one), so results would
come to depend on `N`: batching would become semantically load-bearing instead of
transport-only. The draft's rule is "batch sizes are allowed to vary precisely because no
program can observe them"; an exact static size breaks both halves of that.

So `N` has three defensible readings, and the round is asking which:

- **Absent entirely**: `Batch<T>`; the batching function takes no size at all, and batch
sizes are wholly the transport's choice. Maximal unobservability; no size info for the
compiler.

- **A runtime hint, not in the type**: `batch(s: Stream<T>, n: Int) ->
  Stream<Batch<T>>`, where `n` is "at most n per batch", a hint the transport may ignore,
  and a value (not a type). A program can ask for a size but cannot observe one. This is
   the maintainer's `[Stream<T>, N] -> ...` read at value level, the reading that keeps the
   law intact.
- **A compile-time capacity bound, in the type**: `Batch<T, N>`, `N` meaning "at most
  N", erased at runtime, present for the compiler (allocation sizing, kernel width). This
  is the literal reading of the sketch, and it costs type-level naturals. `N` must be
  ocumented and checked so it can never be read as "exactly", or the law collapses.



## The batching function, and what initiates batching

The function shape follows from the type decision. The smallest consistent proposal:

```toylang
fn batch[T](s: Stream<T>, n: Int) -> Stream<Batch<T>>
```

a prelude builtin, symmetric in shape to `collect` (a stream in, a different stream
out). `n` is optional-to-the-runtime (a hint; the last batch may be shorter; a backend
with a different reader may ignore it entirely). A no-argument `batch(s)` variant, or a
`batch` operator, are spellings the round can rule on but do not change the type story.

What initiates batching is the third open question the maintainer named, and the three
candidates are not rivals -- they compose, and the round should say which may happen:

- **Explicit marker**: batching happens exactly where you write `batch(...)`. Everything
  else stays `Stream<T>`. This is Flink's posture (you must write `.window(...)` before any
   windowed function: the windowing call is the only way windowing starts).

- **Implicit at certain constructs**: named constructs batch as part of their meaning,
  with no call visible -- the stdin reader, and (if it exists) a `gpu(...)` offload site.

  This is Beam's bundle posture (the runner batches, you do not, at the transport
   level). The type still records `Stream<Batch<T>>`, so the program can tell batching
   happened, but no `batch` call appears at those sites. Cost: you must know which
   constructs batch.



- **Forced for certain operations**: operations that need the whole input or a two-level
  reduction (`reduce` with an associative combiner, blocking ops like `sort` over a
   stream) accept only a `Stream<Batch<T>>` and refuse a plain `Stream<T>` with a compile
   error naming the fix (`batch` first). This is exactly the posture `sort` already has over
   `Vec` (needs one, refuses a stream, at compile time), generalized to batching. It
   makes batching the price of those operations, not a choice. This is Spark/Flink's
   posture for shuffles and windowed aggregations (you cannot group or window-aggregate a
   bare stream; the operation requires the structure first).



## The survey: how three chunked-stream systems expose batch boundaries

###Spark partitions: boundaries exist, are runtime-observable, and absent from the static type

An RDD is partitioned across workers; partitions are how parallelism works ("Spark will
run one task for each partition"). The partition count and boundaries are not in the static
type: `distData: RDD[Int]` carries no partition count or scheme. They are observable at
runtime, though:

```python
data = [1, 2, 3, 4,  5]
distData = sc.parallelize(data, 10)    # second arg cuts the dataset into 10 partitions
```

```python
distData.mapPartitions(lambda it: [sum(it)])   # func is Iterator<T> => Iterator<U>
```

`mapPartitions` runs separately on each partition and hands user code the whole partition
as an iterator -- batch boundaries are visible to user code by construction. Partition
indices arrive via `mapPartitionsWithIndex`, and the count via `rdd.getNumPartitions`. Default
partitioning is implicit from the source (one partition per HDFS block, "you cannot have
fewer partitions than blocks"), while `repartition`/`coalesce` change it explicitly. So
Spark's model is the opposite pole from the toylang law: programs may observe batch sizes,
so results can vary by how the input was partitioned, and platform-independence is not
guaranteed. The useful observation for toylang is what Spark does *not* do: it does not
track partitioning in the static type, because partitioning is a physical/runtime concern
the optimizer adjusts (coalesce after a filter shrinks data, etc.). A type-level batch size
would be exactly the thing Spark avoids, for the same reason the toylang draft wants sizes
unobservable: they are the transport's business.



###Flink windows: explicit in the program, staged in the API chain, size not typed, boundaries semantically meaningful

Flink's windowing is the inverse of Spark's partitions: it is explicit, typed as a
pipeline stage, and semantically meaningful rather than transport metadata. The shape is
forced: you cannot window-aggregate a bare stream.

```java
input
    .keyBy(<key selector>)
    .window(TumblingEventTimeWindows.of(Duration.ofSeconds(5)))   // assigner, required
    .reduce(<window function>);                                        // function, required
```

After `.window(...)`, the stream is a distinct intermediate type (`KeyedStream` ->
`WindowedStream`) and the window function receives the whole window as an iterable -- elements
within a window are not individually processable. So the *fact* of windowing is in the
API's type chain, but the *size* is not: `TumblingEventTimeWindows.of(Duration.ofSeconds(5))`
is a runtime/configuration object, and a `WindowedStream<T>` does not parameterize over
window size. Boundaries are observable, deliberately: every element carries a
timestamp, window membership is determined by time (event or processing), and the window
assigner's schedule is known to the program. That observability is semantic (windows are
what you are computing over, not a transport detail), not a leak. What toylang borrows:
windowing is initiated explicitly, and it is *forced* for windowed aggregation (you
must call `.window(...)` before `.reduce(...)`; no bare-stream windowed reduce exists). That
is the "explicit marker" and "forced for certain operations" pair, in a real system.



###Beam bundles: boundaries exist, are the runner's choice, and invisible to user code

Beam processes elements in *bundles*, and the model is explicit that bundles are the runner's
decision:

> The division of the collection into bundles is arbitrary and selected by the runner.
>
> -- [Beam execution model](https://beam.apache.org/documentation/runtime/model/).

Bundles exist for persistence and retry granularity ("persist results after every element, vs
having to retry everything if there is a failure"), not as a semantic or type-level
construct. User `DoFn` code processes elements one at a time (`@ProcessElement`); there is
no bundle in the API surface of a `DoFn`, no bundle type, no bundle size. A runner may
process a nine-element collection as two bundles of five and four, and the number varies by
runner and mode. So Beam's transport batching is toylang's ideal in spirit: boundaries
are unobservable, platform-independent, and never named in the type. But Beam does not
even record *that* batching happened; that is the half-step toylang's `Stream<Batch<T>>`
adds: the type says "batched", while nothing in the type (or the API) can say "how
batched" or "where, exactly, the last boundary was".

Beam also has user-facing batching, kept deliberately separate from bundles:
`BatchElements` and `GroupIntoBatches` are explicit transforms with a runtime batch size,
and a newer *Batched DoFns* feature lets a `DoFn` declare it operates on whole batches
(numpy/pandas/pyarrow types) for performance. That split is worth naming: Beam keeps
its transport batching (bundles) invisible and its user-facing batching (explicit
transforms) typed, and never conflates them. The toylang question "what initiates
batching" is the same split, asked at the language level: transport batching (the
reader, the backend) should be invisible; user batching (a `batch` call, a construct
that batches) is what needs a marker, if it exists at all.



###What the survey settles, in one table

| | boundaries in the static type? | boundaries observable at runtime? | initiation explicit in the program? | what a boundary is for |
|---|---|---|---|---|
| Spark partitions | no | yes (`mapPartitions`, `getNumPartitions`) | default implicit; `repartition`/`coalesce` explicit | parallelism, shuffle co-location |
| Flink windows | stage-typed (`WindowedStream`), size not typed | yes, semantically (timestamps, schedule) | yes (`.window(...)` required, and aggregation forces it) | what you compute over |
| Beam bundles | no | no | no (runner-decided) | persistence, retry granularity |
| toylang proposal | **yes (`Stream<Batch<T>>`), size never** | **no, by construction** | **open question (this round)** | kernel launch, two-level reduction |

None of the three systems puts batch size in the static type, and Spark's runtime
observability is exactly the failure mode toylang's law exists to avoid. The design space
this round is choosing among is better named by the three initiation policies than by
anything the survey shows about sizes: every real system is "explicit" (Flink), "implicit"
(Beam bundles), or "forced" (Flink aggregation, Spark shuffles) somewhere, and
toylang has to say which of those may create a boundary.



##What this spike does not settle

- **Whether `batch` takes a size at all.** A no-argument `batch(s)`, a runtime hint
  `batch(s, n)`, and a type-level `Batch<T, N>` are the three readings of the maintainer's
  `N`;the spike recommends the runtime-hint reading, but the spelling and the
   param are round questions, not something writing more code settles.
- **Which constructs, if any, batch implicitly.** `gpu(...)` is named in the draft as the
  offload construct, but it is unbuilt syntax (`src/` has no `gpu`; the draft reasons about
   what it *would* mean on each backend). If implicit batching is ratified, the construct
   list still has to be chosen (reader only?, reader plus offload sites?, anything else)..
- **How "declared associative" is spelled.** Round 4 ratified the requirement, not the
  syntax: whether `+`'s associativity is declared once on the operator, or each
   reduction writes an `assoc` marker, is still open. This spike assumes the former (it
   makes `sum` over a batched stream read naturally, and the check is one declaration per
   operator, not per call).
- **What `Batch` does to the six backends' codegen.** This is a type-level and
  checker-level design;the emit side (how each backend fuses a batched loop) is the
   follow-up build, not part of the shape question.



##Recommendation for the re-ask

Present the richer effect type as the shape the maintainer asked for, with the size
question and the initiation question as explicit parts of each option, not left implicit.Lead
with **Option A** (opaque `Batch<T>`, runtime hint on an explicit `batch`, batching
forced at the reader and at operations that need a whole input or a two-level reduction): it is
the smallest shape that keeps batch size unobservable by construction, it inherits
round 4's associativity ruling as a property of the type rather than a per-operation gate,,
and it matches what the draft already says ("the input reader batches, and its batching
scheme appears in the type" -- now with a type that cannot leak the size). Present **Option
B** (capacity-typed `Batch<T, N>`) alongside it as the literal reading of the
maintainer's sketch, worth naming explicitly because it is the option that looks like the
obvious next step until the const-generics cost and the exact-N hazard are named. Present
**Option C** (implicit at constructs, no user-facing `batch`) as the honest "batching
is the transport's business" pole, which the Beam bundle model shows is a real posture,,
and say what it costs: no way to chunk a generated pipeline by hand; reductions over plain
streams stay exactly as today (collect first).



Derived: the `Stream`/`Sink` rules from [ADR 0001](../docs/adr/0001-stream-is-the-effect-layer-typed.md)
and the draft's batching section ([the admissible input set, and where batching comes
from](../draft.md#the-admissible-input-set-and-where-batching-comes-from));round 4's
associativity ruling from [board-archive.yaml](board-archive.yaml); the batch-typed
shapes from the maintainer's round-4 answer (recorded there);the Spark, Flink, and Beam
claims from their cited documentation (links in the survey sections; `getNumPartitions` and
`mapPartitionsWithIndex` from the Spark RDD API, not the guide page fetched). Agent-invented:the
`Batch<T>`-has-no-operations formulation, the three readings of `N`, the option packages in
the re-ask round, and the "naive mean becomes unwritable" framing of what the opaque type
buys.