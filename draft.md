# toylang design draft

**Status: exploratory. Everything here is provisional.** This is a thinking document, not a
specification. Syntax is illustrative; several core decisions are still open (see the end).

## What this is

A compiled, statically typed language derived from the jq family.

- **Data-oriented.** JSON is the native value model, not a library.
- **Compiled**, with more than one backend: native, JavaScript, and Lua (the last so it can
  script game engines).
- **Rust-like syntax**, closer to Go in simplicity.
- Aimed at three uses that share one shape: data transformation, shell scripting, and
  result-set-oriented tooling such as an editor whose buffer is a query result.

The front end is written from scratch. See `plans/prototype_1.md`.

This reverses an earlier plan to fork jaq's front end and replace only its interpreter. That
plan rested on two things and both failed once the design settled. The ~640-assertion test
corpus only mattered while jq compatibility was a target, and it is now a non-goal, so it is a
suite nobody is trying to pass. And jaq's parser and 28-node `Term` IR encode jq's *surface
syntax*, while this language's syntax is Rust-like, so almost nothing survives the translation.

What remains true from that analysis is the diagnosis rather than the remedy. jaq's interpreter
allocates a boxed iterator per evaluation step, and that allocation is the performance ceiling a
compiler removes. jq stays a reference for semantics, not a conformance target.

## Two guiding principles

**1. Do not erase boundaries.** When a structural distinction exists, keep it in the type
rather than flattening it away. If a computation crosses a boundary, the crossing is written
down. Corollaries appear throughout: `[...]` is an explicit operator rather than an implicit
coercion, a tuple of streams beats a concatenated stream with a phantom type parameter, and
`Json` is a named type rather than a permissive escape hatch.

**2. Symmetry.** The type-level guarantee and the runtime guarantee must be the *same*
guarantee. If the type system claims two things are distinguishable, the runtime must be able
to distinguish them, and vice versa. A guarantee that exists only at one level is a bug in
the design.

## The core idea: two layers

### Vocabulary

Four words are used precisely throughout this document.

**Cardinality** is how many values something produces. Not *which* values, only how many:
exactly one, zero-or-one, zero-or-more.

**Value layer** means multiplicity is stored *in a value*. An array of three things is one
value that happens to contain three.

**Effect layer** means multiplicity is a property of *evaluation*. An expression that yields
three things is not a container; it produced three results. The name is borrowed from effect
systems, where an effect describes *how* an expression evaluates rather than what it returns.

**Reify and reflect** are the two directions of crossing between layers. Reify means "make it
a thing": collect evaluation multiplicity into a value. Reflect is the opposite, taking a
value's contents and turning them back into evaluation multiplicity.

### The idea

jq's defining feature is that every expression produces a *stream* of 0..n values. The insight
this design rests on is that a stream and a collection are the same information stored in two
different places:

```
[1,2,3] | map(select(. > 1))     # -> [2,3]     multiplicity became array length
[1,2,3] | .[] | select(. > 1)    # -> 2, 3      multiplicity stayed in the evaluation
```

Same computation, same values. The only difference is *where the multiplicity lives*.

So there are two layers, and **exactly two operators cross between them**. These are the only
layer shifters in the language. Nothing else moves between layers, and neither one is ever
inserted implicitly:

```
.[]       value  -> effect      # reflect
[ ... ]   effect -> value       # reify
```

Everything else stays in its layer. `select`, `..`, `,` and `//` live in the effect layer;
`length`, `+` and `sort` live in the value layer; `|` composes within a layer.

This is why `map` and `select` do **not** need to be different kinds of thing. `map(f)` is
sugar for `[ .[] | f ]`, which is reflect, apply, reify. It is cardinality-polymorphic in its
argument and cardinality-collapsing in its result, which is exactly why `map(select(p))`
typechecks and returns a plain array.

### Relationship to "index vs. iterate"

These are related but not identical distinctions, and how they relate is currently an open
question (see open question 1).

Layer is about where multiplicity is *stored*. Index versus iterate is about what access a type
*promises*: constant-time positional access, or sequential walking only.

If multiplicity in the effect layer is purely an evaluation-time phenomenon, with no stream
*values* existing at all, then the two distinctions collapse into one. The effect layer is then
the only iterate-only thing, and `Vec` is the only multiplicity-bearing value.

If streams are instead first-class values, there are three things to distinguish: `Vec` (a
value, indexable), `Stream` (a value, iterate-only), and effect-layer multiplicity. The
distinctions then stay separate and both must be tracked.

**This document has not yet chosen**, and two sections below are written from opposite
assumptions. The cardinality table presents `Stream<T>` as a type, which presumes first-class
streams, while this section treats multiplicity as evaluation-level. That tension is the open
question showing through, not an oversight to be papered over.

## Values

```
null   true   42   3.14   "text"   [1, 2, 3]   {name: "ada", age: 36}
```

The JSON value forms, with `Str` a real string type rather than an untyped blob.

## Cardinality is part of the type

```
T            exactly one
Opt<T>       0..1
Vec<T>       0..n, finite, indexing without iteration
Stream<T>    0..n, possibly infinite, indexing only by iteration
```

`Vec` and `Stream` are not "eager" and "lazy". The honest difference is the **promise they
make**. `Vec` guarantees indexing without iteration; `Stream` does not, and may be infinite.
That asymmetry is load-bearing, as `fold` below shows.

TODO: explore idea that Vectors are multidimensional, so their cardiniality is expressed per-dimension;
the foo[] operator is actually just applying another vector as a projection on a vector or a stream, so that would be
a "project" trait. (I guess we could have a convention that traits are methods with __ as prefix and suffix liek in python
so basically, if you havea  vector "foo", that is one dimensional, then foo[3] would be just getting the third item. the projectsion itslef is actually the vector `[3]`.
The filter `.[3]` would be getting the third item of each value in the stream. If a 2D vector is projected by a vector of length one, then you'd get not single items, but a one dimensional vector.
there could also be some elegant way of specifying when you want to project not along the first dimension, but perhaps that would be an operator like `.{1: 3}` which woudl mean that get the subvector
where there is one less dimension and only the items where the orignal dimension 1's coordinate is "3" are returned. but I guess that these projections coudl become lenses, rahter than hard copies in
memory and rather than beocming streams since they have fixed cardinality, even if that si also only derived from their "parent" object. This means that even `.[]` only becomes a stream if `.` was already a stream, I guess. but when to materialize is another question: i guess that `.[] | select ...etc ...` would be allowed to materialiez teh resutl instead of making it a lens, even tho it could appear like a lens in the type system, that is if we can assume that no more references to `.` exist elsewhere.

RESPONSE (agent). Four things this does, one of which breaks something above.

It answers "index vs. iterate" by dissolving it. If `[]` is projection by a vector, then `foo[3]`
and `foo[]` are one operator with different projections, `[3]` and "all indices". That is a
better answer than treating them as two access modes, and it is what NumPy and APL do.

It contradicts the two-layer section. That section says `.[]` is *the* reflect operator, the
thing that turns a value into effect-layer multiplicity. Under projection-with-lenses, `.[]` on
a `Vec` yields a `Vec` view with known cardinality, which never leaves the value layer. So
either `.[]` stops being a layer shifter, or projection is a different operator that happens to
share syntax. Worth noting the version where `.[]` stops shifting layers is arguably *better*:
the effect layer would then arise only from genuinely unknown cardinality (I/O, `select`, `..`),
and everything with statically known extent stays a value, which is also exactly what
vectorizes. But the claim "exactly two layer shifters" does not survive it unchanged.

--> but is this a true contradiction, or is this somethign that actually a trait that is implemented
over everythign that has the index trait, and separately over anythign that has the iteration trait?

but perhaps the implementations on both can actuallyb e slightly different, as there may be some edge cases that do not work in both

RESPONSE (agent). The trait framing works, and it resolves more than it claims. Not "this is
not a contradiction" but "`.[]` was never the layer shifter in the first place."

Follow it through. If `__project__` has an impl for indexable things and another for iterable
ones, then `Vec[]` returns a `Vec` view and stays in the value layer. So `.[]` does not create
effect-layer multiplicity at all. What does? Only things with genuinely unknown extent: I/O,
`..`, and `select`.

Except that under the columnar model `select` does not either. A predicate over a batch is a
mask, and masking a `Vec` gives a `Vec`. So it also stays in the value layer.

Keep pulling and the effect layer shrinks to one thing: genuine streaming input whose length is
unknown because it has not arrived yet. That is a much smaller and much more defensible notion
than "anything with more than one value", and it would mean there is one layer shifter rather
than two, running one way, `[...]` collapsing a stream into a value.

The two impls differing is fine and expected. `Vec[3]` is constant time and yields a view;
`Stream[3]` consumes three items and cannot yield a view. Different cost, different guarantee,
same trait. Symmetry is satisfied because the type says which one you got.

### PROPOSAL: the layer shift only runs one way

If `.[]` is no longer the reflect operator, the obvious next move is to find a replacement. Two
candidates came up, and there may be a third that is better than both.

**Projection yields an iterator of lenses.** Fails for the reason it was raised with: an
iterator does not carry its length in its type, so cardinality is lost the moment you have one.
Recovering it would need the length in the type, `Iter<T, n>`, which is dependent typing.

**Keep `.[]` as the lossy layer shifter, add separate syntax for the cardinality-preserving
lens.** This works, and it is honest: these are two different operations that jq conflates only
because jq has neither lenses nor cardinality tracking. But it puts the *default* on the wrong
side. The familiar, short, habit-forming spelling would be the one that throws information away,
and the information-preserving one would need a deliberate reach. That inverts principle 1.

**Third option: there is no value-to-effect shifter, because it would only destroy
information.** Ask what converting a `Vec` into effect multiplicity actually buys. Effect-layer
multiplicity exists to describe extent that is not yet known. A `Vec` already knows its extent.
Degrading it forgets that and returns nothing in exchange, since laziness cannot help with data
that is already materialized.

Under this reading, effect multiplicity is *born* from streaming sources whose length has not
arrived yet, and *dies* into values through `[...]`. It never arises from a value. There is one
layer shifter, running one direction, and the absence of the other direction is not a gap.

Check it against real pipelines. `vec | map(f) | select(p)` is value layer throughout. `first(vec
| select(p))` is `Vec -> Vec -> Opt`, still value layer. `stdin.lines | select(p) | [...]` is
born effect, collapsed to a value. Writing a `Vec` to stdout iterates internally and needs no
type-level stream. Avoiding an intermediate materialization is fusion, which is a compiler
concern and does not change the type.

Two consequences if this holds.

It answers Q1. If effect multiplicity is only ever born from I/O and only ever collapses into
values, there is no need for a `Stream<T>` *type* at all. `Stream` becomes an effect annotation
on an expression rather than a type constructor, which is the evaluation-level answer arriving
for a fourth time.

It simplifies Q7. If `..` is value-layer, producing a collection of nodes, then whether it
promises depth-first order is an ordinary question about how a value is ordered, rather than a
question about evaluation strategy.

The case that would break it: any value with genuinely unknown extent. That is exactly what a
first-class `Stream` value would be, so Q1 and this proposal stand or fall together.

### Does a value-layer `select` copy?

Asked of `vec | map(f) | select(p)`, which the section above claims never leaves the value
layer. If `select` returns a `Vec`, does it allocate a filtered one?

No. It returns the original buffer plus a **mask**, so no elements move. That is what the
columnar model means by producing a bitmask rather than branching. It also makes `select` a
projection, projected by a boolean mask instead of an index vector, which brings it under the
same `__project__` trait as `[]`.

But it breaks the promise that defines `Vec`. `Vec` is "indexing without iteration", and
indexing a mask-filtered view is not constant time: finding the seventh surviving element means
finding the seventh set bit, which is a rank query, and rank is O(n) unless you build an index
for it.

The two standard representations trade off differently, and both are used:

```
bitmap            1 bit per SOURCE element.  Cheap to AND together.  Indexing needs rank.
selection vector  32 bits per SURVIVING element.  Indexing is a gather, so constant time.
```

Vectorized databases switch between them based on selectivity, because a bitmap wins when most
rows survive and a selection vector wins when few do. Note the asymmetry this creates with the
projection proposal: projecting by an index vector preserves constant-time indexing, because it
is a gather, while projecting by a bitmask does not.

So there are three positions and the choice is open:

1. `select` returns a weaker type that knows its length but does not promise constant-time
   indexing. This is a real third thing between `Vec` and unknown extent, and admitting it makes
   the promise lattice richer than two entries.
2. `select` returns a selection vector, keeping `Vec`'s promise intact, and pays memory
   proportional to survivors rather than to sources.
3. `select` materializes, which is the only option that copies, and is sometimes right when
   selectivity is very low and the result outlives the source.

One cost is shared by 1 and 2 and worth stating plainly: a masked or index-projected view
**keeps the entire source buffer alive**. Filtering a million records down to three and holding
the result retains all million. This is the same hazard as a substring that pins its parent
string, and it has bitten Java and Go. Whatever else is decided, materialization needs to be
reachable on purpose.

Per-dimension cardinality assumes rectangularity, and JSON is ragged. `[[1,2],[3]]` is not a 2D
vector; it is a nested list with variable inner lengths. Arrow handles that with offsets, not
dimensions. So this probably needs two distinct types rather than one: a rectangular
`Tensor<T, dims>` where per-dimension cardinality is meaningful, and a ragged `Vec<Vec<T>>`
carried as offsets. The projection algebra is clean on the first and only partly applies to the
second.

---> yes, but we can actually differentiate between types that actually promise the matrix format, vs. types that don't. so simply a regular json vector will not be multidimensional (if we add the zod-inspired parse system later, that might change but only through a runtime check boviously, so it will be a branching. this makes sense. no?

RESPONSE (agent). Yes, and the parse framing is the part that makes it good rather than merely
workable. Rectangularity is *earned*, not assumed:

```
as_tensor : Vec<Vec<Num>> -> Result<Tensor<Num, [n, m]>, ShapeError>
```

That is parse-don't-validate applied to shape. The ragged type is what JSON gives you, the
rectangular type is what you get after a check that can fail, and the check is a named operation
rather than an implicit coercion. The boundary between ragged and rectangular is explicit and
crossed on purpose, which is principle 1 doing real work rather than being quoted.

It also means the projection algebra does not have to apply to ragged data. It applies to
`Tensor`, which is the type that promised the shape it needs.

The materialization condition is a uniqueness analysis. "If we can assume no more references to
`.` exist elsewhere" is linearity or uniqueness typing, which is Rust's borrow checker or Clean
and Idris 2. It is not free, and it is the machinery we avoided needing for ordering guarantees.


---> yes, but this language is allowed to have also a garbage collector - so perhaps the answer is to pick the garbage
collector algo in such a way that it basically has a layer for the "i want to override this if it woudl be garbage colleted anyway, otherwise please give me a place i can put my mapped copy"|

RESPONSE (agent). This is copy-on-write with a refcount-of-one check, and it is what array
languages actually do. Swift exposes it as `isKnownUniquelyReferenced`, R has it as NAMED and
later true refcounting, Rust spells it `Rc::make_mut`, and APL and J implementations have done
it for decades for exactly this reason. Landing on it independently is a good sign.

Two costs worth naming before adopting it.

It trades a static guarantee for a data-dependent one. Static uniqueness typing promises at
compile time that no copy happens. A runtime check promises only that a copy happens when
needed, so whether a program allocates depends on aliasing the type system never reported. That
collides with principle 2 and with Q8: the type would say "view", and the runtime would
sometimes copy. Defensible if performance is deliberately excluded from what the type
guarantees, but that should be a decision rather than a side effect.

More concretely, the check needs reference counts to exist. A tracing collector does not
maintain them, so "is this uniquely referenced" is not a question it can answer cheaply. So this
choice quietly votes for refcounting, or for a hybrid that refcounts for the uniqueness check
and traces to collect cycles, which is roughly CPython's arrangement.

That connects to a decision already implied elsewhere. jq values are immutable and acyclic, so
refcounting alone is complete for them. Cycles only become constructible once mutable cells
exist. So the mutation model and the copy-on-write question are the same decision viewed from
two sides, and settling one settles the other.

One connection: adopting projection semantics pulls open question Q2 (cartesian vs. zip) toward
broadcast, because multidimensional projection and elementwise broadcast are the same tradition.
Q2 and this TODO may be one question.

Under the hood these are all the same shape, unpacked a different number of times. Unpacking a
sequence yields either *nothing* or *one item plus a remainder*, written `1 + T*X`, where `1`
is the nothing case, `T` is the item, and `X` is the remainder. That template is called the
**base functor**:

```
Opt<T>     =  1 + T           (no remainder, so it stops after one)
Vec<T>     =  muX. 1 + T*X    (least fixpoint: must terminate, so finite)
Stream<T>  =  nuX. 1 + T*X    (greatest fixpoint: need not terminate, so possibly infinite)
```

**Fixpoint** here means solving `X` by substituting the definition into itself. The least
fixpoint (`mu`) admits only finite solutions; the greatest (`nu`) also admits infinite ones.
That single choice is the entire difference between an array and a stream.

One definition, three types. The finite/infinite split is *derived*, not stipulated.

## Functions are unary

Multi-argument means "takes a struct", with inline construction so it stays invisible:

```
fn adults(db: Db) -> Stream<User> =
    db.users[] | select(.age >= 18)

fn limit(a: {count: Int, over: Stream<T>}) -> Stream<T> = ...

limit {count: 3, over: repeat(1)}        # -> 1, 1, 1
```

A consequence worth noting: jq needs `;` to separate filter arguments because `,` is already
the effect-layer operator. With only unary functions there is no argument list to disambiguate,
so `,` stays free and `;` is unnecessary.

### Annotation rule: named functions declare, lambdas never do

**Named functions must annotate their parameter and return types. Lambdas must not, and
cannot.** A lambda gets its type from the position it appears in.

This is **bidirectional type checking**, which has two modes. *Synthesis* works bottom-up:
given an expression, work out its type, so `42` synthesises `Int`. *Checking* works top-down:
given an expression and an expected type, verify it fits.

Annotations on named functions are the seeds that synthesis starts from. Lambdas are always in
checking mode, because a lambda only ever appears somewhere that already knows what it wants:

```
fn map(a: {over: Vec<A>, with: A -> B}) -> Vec<B> = ...

users | map(|u| u.name)
#           ^^^^^^^^^^ checked against `A -> B`
#                      A = User is known from `over`, so `u : User` needs no annotation
#                      B = Str is learned from the body
```

Nothing is inferred in the hard sense. The signature of `map` already fixed the shape; the
lambda is only verified against it.

Most lambdas are unnecessary anyway, because `.` is the implicit subject:

```
users | map(.name)        # no lambda at all
users | map(|u| u.name)   # identical, just named
```

When there is no expected type, it is an error rather than a guess:

```
let f = |x| x + 1                  # ERROR: nothing here says what x is
let f: Int -> Int = |x| x + 1      # fine, the annotation supplies it
```

This is the deliberate part. A language that guessed here would have to invent a type from the
body, which is where inference becomes unpredictable and error messages start pointing at the
wrong line.

Nesting propagates inward without any extra machinery:

```
groups | map(|g| g.items | filter(|i| i.ok))
#              ^ from map's signature   ^ from filter's signature
```

Returning a lambda works because the named function's return annotation supplies the type:

```
fn adder(n: Int) -> (Int -> Int) = |x| x + n
#                   ^^^^^^^^^^^^ this is what types the lambda
```

Named functions are required to annotate for two reasons. The first is recursion. Inferring the
type of a recursive function without a declared signature requires polymorphic-recursion
inference, which is undecidable in general, and the standard fix used by OCaml and Haskell is
exactly this annotation requirement.

```
fn depth(t: Tree) -> Int =            # `-> Int` is what makes this checkable at all
    1 + max(t.children[] | depth)
```

The second is that it keeps checking **local**. Every function can be checked knowing only the
signatures of what it calls, never their bodies. That means fast compilation, and error
messages that point at the mistake rather than at some distant unification failure.

None of this costs terseness, because annotations only ever appear at named-function
boundaries, which one-liners do not have:

```
stdin.lines | parse_json? | select(.level == "ERROR") | .service
```

Not one type annotation, and every step is still fully checked.

## Field access is a lens

A **lens** is a first-class reference to a *position inside* a structure, not the value there
but the place itself. Because it names a place, it supports reading, writing, and being
reported as a path, all from one expression.

A path expression is therefore simultaneously a getter, a setter, and a path witness. That is
what makes update-in-place (`|=`), deletion, and path enumeration possible over the same
syntax, so `.foo` desugars to a lens rather than a getter:

```
trait Field<K> {
    fn get(self, k: K) -> Self
    fn path(self, k: K) -> PathPart
    fn set(self, k: K, v: Self) -> Self
}
```

jq conflates "missing" with "type error": `null.a.b.c` yields `null` but `1 | .a` raises. Those
are genuinely different outcomes and the language should distinguish them:

```
user.name           # User has `name` -> typechecks, exactly one Str
user.nmae           # COMPILE ERROR, no such field
json.name           # Json's fields are all optional -> Opt<Json>
json.name!          # unwrap, or propagate the error
```

Three distinguishable outcomes: a value, a *specific* absence, a *specific* error.

## Query is search

**Nondeterminism** here does not mean randomness. It means an expression denotes *a set of
possible answers*, and evaluation explores all of them in a fixed order. This is the sense used
in Prolog and in nondeterministic automata. Formally it is the *list monad*: a filter maps one
value to a list of results, and `|` chains those lists together.

Two search terms used below. **Cut** means committing to what you have and abandoning the
remaining alternatives. **Pruning** means discarding a branch before exploring it.

| operator | search meaning |
|---|---|
| `\|` | bind, so for each choice, go deeper (depth-first descent) |
| `,` | choice point, so branch wider |
| `empty` | dead end, backtrack |
| `first(f)` | cut |
| `..` | full tree traversal |
| `select` | pruning |
| `//` | alternatives, left to right |

```
..                                 # every node, depth-first
.. | select(.kind == "error")      # prune to matches
first(.. | select(.id == 7))       # stop at the first hit
.a // .b // "default"
```

A reified search is a result set:

```
fn diagnostics(tree: Ast) -> Vec<Diag> =
    [ tree.. | select(.kind == "error")
             | {file: .loc.file, line: .loc.line, msg: .text} ]
```

## Single-pass composition

A `Stream` can only be walked once, so combining several independent accumulations over one
stream needs first-class support:

```
fn stats(xs: Stream<Int>) -> {sum: Int, count: Int, max: Int} =
    fold xs {
        sum:   0    with (acc, x) -> acc + x
        count: 0    with (acc, _) -> acc + 1
        max:   MIN  with (acc, x) -> max(acc, x)
    }
```

Three folds, one iteration, one struct out. A **fold** is an accumulation over a sequence, so
`sum`, `count` and `max` are all folds. The trick here is that several folds are declared
independently but *run together in one pass*, usually called making folds **applicative**,
meaning they combine without being sequenced. Haskell's `Control.Foldl` is the reference
implementation.

`Vec` does not need this, since you can simply walk it three times. The construct exists
*because* `Vec` and `Stream` make different promises, which is the type system earning its keep
rather than decorating.

### `reduce` and `fold` are different operations

jq's `reduce .[] as $x (0; . + $x)` is a left fold with an explicit accumulator. It is
**order-defined**, and reassociating it changes results, so it cannot be parallelised or
vectorised. Rather than annotate that away, keep two constructs:

```
reduce   sequential, order-defined, CPU only.  The accumulator is threaded, and you can see it.
fold     declares its operator associative and commutative.  Order is unspecified.
```

The distinction then lives in the source text rather than in a pragma. A reader seeing `fold`
knows the summation order is not promised; a reader seeing `reduce` knows it is. Rust draws the
same line between `Iterator::sum` and `Iterator::fold`.

This earns its keep on the CPU before any GPU exists. LLVM refuses to vectorise a floating-point
reduction, because reassociation changes results, unless the `reassoc` flag is set on the
instructions. Integer reductions vectorise freely, since integer addition really is associative.
So `fold` is exactly the construct where setting `reassoc` is legitimate, and `reduce` is exactly
where it is not. The language-level distinction and the compiler-level flag are the same
distinction, which is principle 2 holding at the machine level.

## Backends, vectorization, and the offload boundary

### Cardinality is the kernel-admissibility predicate

The strongest result in this section. A GPU kernel sublanguage is usually specified by listing
what is banned. Stated in this language's own vocabulary it is not a list at all, because each
cardinality corresponds to a known kernel pattern:

```
One<T>       exactly one output per input   ->  elementwise map kernel
Opt<T>       zero or one                    ->  stream compaction (prefix sum)
fold                                        ->  reduction
Stream<T>    unbounded, unknown extent      ->  NOT admissible
```

So the offload boundary and the layer boundary are the same boundary. Everything in the value
layer is a candidate; the effect layer is exactly what cannot be a kernel. Nothing new has to be
invented to say which programs can run on a GPU, because the cardinality effect already says it.

What still has to be excluded inside an offloaded region is the ordinary list: strings, objects,
path expressions, update assignment, `error`, and `input`.

#### This contradicts an earlier claim, and the earlier claim was wrong

Q8 was argued on the grounds that cardinality and vectorizability are *orthogonal*: `select`
changes cardinality and vectorizes fine as a mask, while `first` changes cardinality the same way
and cannot vectorize at all. If cardinality is the admissibility predicate, that counterexample
has to go somewhere.

It goes away, because it equivocates on `first`. There are two of them:

```
first over a Stream   must short-circuit over data that has not arrived.  NOT admissible.
first over a Vec      is the minimum index where the mask is set.  A reduction.  Admissible.
```

So `first` is not one operation that defies the mapping. It is two operations in different
layers, and only the streaming one is inadmissible, which is exactly what the `Stream` row
already says. Admissibility is determined by the cardinality of what an expression *consumes*,
not only by what it produces. The same resolution covers `any`, `all`, and short-circuiting
`and` and `or`: over a `Vec` each is a reduction over a mask, and over a `Stream` each is an
early exit.

#### But the mapping needs a precision about granularity

It describes the cardinality of a filter applied *per element*. It says nothing about operations
on a whole collection, where `Vec -> Vec` is one value in and one value out, and the per-element
reading does not apply.

`sort` is the clear case. Its cardinality is one-to-one and it is not an elementwise kernel.
Neither are `group_by` or a join. These are the blocking operators, they need the whole input
before producing anything, and they are parallelizable by different means entirely. So the
mapping classifies elementwise filters, and whole-collection operators are a separate question
this document has not addressed.

### The admissible input set, and where batching comes from

A cleaner statement of the boundary, and the one this document adopts. Admissibility is a
property of the **type**, not of the operation:

```
admissible    scalars, and anything of known cardinality
              i.e. one pre-allocated buffer plus a little lens metadata
inadmissible  streams.  Compile error, not a silent fallback.
```

The lens metadata is what avoids copying. A projection does not have to be materialized before
launch, because the kernel can recompute addresses from the lens parameters on the device. That
is ordinary strided or affine indexing and it is what makes views free rather than merely cheap.

Streams are rejected outright and must be materialized first. That is the design working rather
than a limitation: **reification is where allocation becomes visible in the source**, so the one
operator that costs memory is the one you have to write down.

Processing a stream therefore means batching, and the important decision is who does it. Not the
language, invisibly. **The input reader batches**, and its batching scheme appears in the type:

```
stdin           Stream<Vec<T>>      batched by the reader, the batching is in the type
your own source Vec<T>              you choose: one big vector, or batch it yourself
```

So the two API levels fall out. Low-level vector and GPU operations take a single `Vec`.
High-level operations take a stream of them. Nothing is magic, because the magic is located in a
reader whose type says what it did.

Four things this exposes.

**Batch size must not be observable.** The whole design rests on it. If a reader picks the batch
size and a program can tell what it picked, results vary by input source and the semantics stop
being platform-independent. So only batch-invariant operations may run over `Stream<Vec<T>>`.
This is the same move as the string design: UTF-8 and UTF-16 are both allowed precisely because
no program can observe which it got, and batch sizes are allowed to vary precisely because no
program can observe them. Worth noticing that the same technique is now load-bearing twice.

**Batching requires the associativity declaration.** A fold over a batched stream is a two-level
reduction, within each batch and then across batches, and that is only sound when the operator
is associative. So `fold` and `reduce` are not merely a vectorization nicety; the batched reader
cannot exist without them. `reduce` over a `Stream<Vec<T>>` has to be either rejected or forced
back through a single sequential pass.

**Known cardinality has two meanings once views exist.** A dense `Vec` knows its extent. A
mask-filtered view knows its *capacity* but its *count* needs a popcount. Both are launchable,
but not by the same path: the second needs a reduction before the output buffer can be sized,
which is precisely the prefix-sum step of stream compaction. So dense and masked probably need
to be distinguishable in the type, since they have different launch preconditions. This is Q14
arriving from the other side.

**What `gpu(...)` means on the other backends.** It cannot be a compile error on Lua and
JavaScript without making programs platform-dependent, which the whole design is trying to
avoid. So it has to be a placement hint that changes *where* something runs and never *what* it
computes, and on a backend with no device it lowers to the ordinary loop. That keeps the earlier
result intact: choosing between kernel and vector loop stays a late decision.

### The same predicate governs CPU vectorization

LLVM's loop vectorizer rewrites a scalar loop to process several elements per iteration with no
annotation, and its SLP vectorizer does the same for repeated straight-line operations. It is
automatic, with three qualifiers: it runs only at `-O2` or above, so the pass pipeline has to be
run rather than merely emitting IR; it has to prove legality; and it has to judge the result
profitable against a target cost model.

The legality blockers, in roughly the order they bite in practice, are aliasing, loop-carried
dependencies, floating-point reductions, calls in the loop body, unknown trip counts combined
with early exits, and non-unit strides.

Every one of those is something this design can guarantee away statically:

| blocker | what removes it |
|---|---|
| aliasing, the most common cause | immutable inputs and a distinct output buffer, so `noalias` is emitable and the runtime overlap check disappears |
| loop-carried dependency | a pure elementwise filter captures no mutable state |
| float reduction | `fold` declares associativity, so `reassoc` is legitimate; `reduce` does not, so it is not |
| calls in the body | a fused pipeline is one loop body with everything inlined |
| unknown or non-unit stride | a dense buffer has unit stride known at compile time |

So the offload check and the "will this vectorize" check are nearly the same predicate. That is
the good outcome: a region either dispatches to a kernel or lowers to a loop that reliably hits
NEON or AVX-512, and choosing between them is a late decision about *where to run* rather than a
semantic fork.

Worth building in from the start: verify rather than hope. `-Rpass=loop-vectorize` and
`-Rpass-missed=loop-vectorize` report which loops vectorized and why the others did not, and a
regression there should fail a test rather than quietly cost throughput.

### Backend choice

Not a question about JSON-shaped types. Both LLVM and Cranelift bottom out in integers, floats,
vectors, pointers, and memory; neither has a string, a collector, or a number tower. Those
semantics live in the front end and the runtime, and the backend never sees a string at all,
only a pointer and a length. If semantics drift across targets it is because they were defined
in the lowering rather than above it.

What does differ:

```
                     LLVM (via inkwell)              Cranelift
SIMD                 vector + scalable vector types  128-bit vector types, well tested
auto-vectorization   loop and SLP vectorizers        NONE, explicit SIMD in and out
GPU                  NVPTX, AMDGPU, SPIR-V, raw IR   none, and not on the roadmap
wasm output          wasm32 target                   none; Cranelift consumes wasm
build                pinned system LLVM, C++ chain   pure cargo
compile speed        slower                          5 to 10 times faster
```

Two things follow. Cranelift never vectorizes for you, which matters a great deal here, because
the whole argument above is that this design can hand a vectorizer exactly the loops it likes.
And if the browser story ever becomes WebAssembly rather than emitted JavaScript source, LLVM
runs the *same* IR through one pipeline for both native and web, which is the strongest available
guarantee that semantics do not vary by platform. Cranelift produces nothing for the web, so that
would need a second unrelated backend and the drift risk would be entirely self-policed.

Cranelift's real advantages are build simplicity and compile speed, which matter for a REPL and
for dev builds. Using both, as rustc does, is a normal answer.

Its GPU story is a genuine architectural mismatch rather than missing work: it assumes an SSA
control-flow graph lowered to a flat instruction stream with a conventional register allocator
and CPU calling conventions, while GPUs need divergent control flow with execution masks, a
multi-level address-space memory model, workgroup and barrier semantics, and register allocation
whose objective is occupancy. LLVM's GPU support is real but thin: it gives you the assembler and
nothing above it, so address spaces, kernel calling conventions, thread-index intrinsics, and
launch are all hand-managed.

### A dense tensor type

The value model gains a seventh kind alongside null, bool, number, string, array and object: a
dense typed buffer. It is constructed explicitly, never inferred:

```
.readings | @f32 | reshape(1024; 3)
```

`@f32` narrows a JSON array of numbers into an unboxed buffer and hard-fails on heterogeneous
input, nulls, or nested strings, at the constructor rather than three stages later. It
serializes back out as nested arrays, so JSON round-tripping survives at the value level.

`@f32` is also the second number type. This language commits to `f64` to match JSON, and `@f32`
is a deliberate lossy exit from that commitment, which is a good reason to make it a visible
operator rather than something a type inferencer decides.

Nulls are the awkward part, since JSON has one and a dense `f32` buffer does not. **Do not use
NaN as a sentinel**, because it collides with genuine NaN. Apache Arrow already solved exactly
this with a separate validity bitmask beside the values buffer, and adopting its layout also buys
zero-copy interop with Polars, DuckDB, and pandas. For a language whose pitch is JSON processing,
that is a large return on a layout decision that has to be made anyway.

Arithmetic on tensors broadcasts, so `$m * 2` is elementwise. That extension is confined to the
new type; plain JSON arrays keep erroring exactly as they do now.

## Strings are where platform independence actually costs something

JavaScript strings are WTF-16: UTF-16 code units, lone surrogates permitted, with `length` and
indexing measured in code units. If the same program must mean the same thing natively and on a
JavaScript target, there are three honest options.

**WTF-16 everywhere.** Exact JavaScript semantics, trivially identical across targets. Pays
memory and a conversion on every C FFI call natively.

**UTF-8 everywhere, with the JavaScript-shaped API emulated.** Cheap and idiomatic natively, but
on the JavaScript target the strings cannot *be* JavaScript strings, which guts interop
ergonomics and forces conversion at every boundary.

**Design the difference away.** Do not expose code-unit indexing or a code-unit `length` at all.
Offer iteration over scalar values and opaque indices instead. Then UTF-8 natively and UTF-16 on
the web are both conforming implementations, because no program can observe which one it got.
This is roughly Swift's move, it is the only option that is cheap on both sides, and it is a
language-design commitment that has to be made early because it constrains the string API
permanently.

The same reasoning applies to numbers, where committing to `f64` everywhere means keeping
floating-point contraction off so the optimizer does not fuse operations behind your back, and to
object key ordering if JSON round-tripping is meant to be stable.

## Mutation

Immutable values plus a small number of explicit mutable cells. Cycles can only form through a
cell, which keeps them syntactically visible.

```
let db2 = db.users[0].name = "ada"    # shadow: db unchanged, db2 is new
let c = cell(0)                        # explicit mutable cell
c <- c.get() + 1                       # in-place write
```

Orthogonal to cardinality.

### UNDECIDED: what to call the product-forming update

In jq, `=` is not assignment. Its right-hand side is an ordinary expression, so if it yields
several values, the whole update yields several results:

```
{} | .a = (1,2)        # -> {a:1}, {a:2}      TWO objects, not one object with two values
```

That is genuinely useful. It gives config-matrix expansion, variant generation, and
property-test input enumeration for free. The problem is purely that `=` *looks* like mutation
while behaving like a product, and the multiplicity is invisible at the call site.

Compounding it, jq's `=` and `|=` disagree about cardinality and say nothing about it:

```
{a:1} | .a =  (1,2)      # -> {a:1}, {a:2}    cartesian
{a:1} | .a |= (.,.+10)   # -> {a:1}           silently keeps only the first
```

Options under consideration:

**A. Keep `=`.** Familiar to anyone arriving from jq, with zero migration cost. But it
preserves exactly the readability problem, and the `=` versus `|=` mismatch stays a trap.

**B. Require `One` on the right, and make forking explicit.** `=` typechecks only when its
right-hand side yields exactly one value, so the surprising case becomes a compile error. When
a product is wanted, it is written out:

```
db.color = "red"                        # ok
db.color = ("red", "blue")              # ERROR: expected One<Str>, found 2 values
("red","blue") as $c | db.color = $c    # explicit; jq already supports this and it reads better
```

**C. Two distinct operators.** `=` for the single-valued case, and a visually distinct one for
the deliberate product, such as `.color =* ("red","blue")` or `.color each= (...)`. Keeps both
without either being silent, at the cost of more surface.

**D. Drop `=` entirely and keep only `|=`.** All updates go through the update operator, and
products come from an explicit `cross` or `for` construct. Smallest core, largest departure.

**E. Rename to a functional-update keyword.** `db with .color = "red"`, in the spirit of record
update in ML-family languages. Removes the mutation reading, but adds a keyword and does not by
itself resolve the cardinality question.

Leaning towards B, because it makes the hazard a type error rather than a naming problem, and
the explicit form already exists and reads better. But this interacts with open question 2,
whether binary operators are cartesian, zipped, or explicit, so it should not be settled alone.

## Two worked programs

Shell, counting errors per service, streaming, in constant memory:

```
#!/usr/bin/env toylang
stdin.lines
  | parse_json?                              # skip malformed lines
  | select(.level == "ERROR")
  | fold {} with (acc, e) -> acc[e.service] += 1
  | to_entries | sort_by(-.value) | .[]
  | "\(.key)\t\(.value)"
```

Editor, where the buffer is a query result:

```
fn view(project: Project) -> Vec<Row> =
    [ project.files[]
      | .diagnostics[]
      | select(.severity >= WARN)
      | {file: ^.path, line: .line, text: .msg} ]

on "]q" -> cursor.next()
on "gf" -> open(cursor.get().file, cursor.get().line)
```

There is no buffer, only a reified search.

## Why cardinality-in-the-type is the safety mechanism

jq's multi-output semantics produce hazards that are all the same shape, namely the right
feature in the wrong position. Measured against a jq implementation:

```
if (true,false) then "a" else "b" end   -> ["a","b"]       BOTH branches executed
{} | .a = (1,2)                          -> [{a:1},{a:2}]   assignment forked the world
{a:1} | .a |= (.,.+10)                   -> [{a:1}]         |= silently took only the first
(1,2) | (., error("boom"))?              -> [1,2]           `?` truncated with no signal
```

None of these is an argument against nondeterminism, because each is genuinely useful
somewhere. Forking on assignment is config-matrix expansion. Both-branches is nondeterministic
choice. Truncation under `?` is "take the valid prefix" of a corrupt stream. What they have in
common is that multiplicity leaked into a position that wanted exactly one value.

Making cardinality visible turns each into a type error at the point of the mistake:

- `if` requires exactly one `Bool`
- a map key requires exactly one `Str`
- anything that runs an effect requires its arguments collapsed (`first`, `only`, `[...]`)

Multiplicity stays free where it is useful, notably in structural positions, where `0..n`
naturally means "this many children."

## Open questions

Tracked here rather than scattered through the document. Status is one of OPEN (no preferred
answer), LEANING (a preferred answer exists but is not committed), BLOCKED (waits on another
question), or SETTLED (answered, and the answer is written into the document above). Add new
ones at the bottom and keep the numbers stable, since other sections cite them.

Settled questions stay in the table. A tracker that only lists what is unresolved cannot be
checked for completeness, and the settled entries are what stop a decision being relitigated.

| # | Question | Status |
|---|---|---|
| Q1 | Streams: first-class values, or evaluation-level multiplicity? | LEANING, evaluation-level; four independent arguments now agree |
| Q2 | Binary operators over two multi-valued expressions: cartesian, zip, or explicit? | OPEN |
| Q3 | What symbol replaces `=` for the product-forming update? | LEANING, blocked on Q2 |
| Q4 | Can the type express ordering over heterogeneous streams? | OPEN, subsumes cardinality-vs-order |
| Q5 | Stream-lowering strategy across the three backends | OPEN, blocks all backend work |
| Q6 | Does a reconciler belong in the language or a library? | OPEN |
| Q7 | Does `..` promise depth-first order, or only the set of nodes? | OPEN |
| Q8 | Is vectorizability visible in the type system, or a silent optimization? | OPEN |
| Q9 | Are vectors multidimensional, with `[]` as projection? | OPEN, may merge with Q2 |
| Q10 | Is uniqueness analysis in scope, for deciding when a lens materializes? | OPEN |
| Q11 | How does the query/transformation split manifest in the type system? | SETTLED |
| Q12 | On a type mismatch, does field access error, yield null, or something third? | SETTLED |
| Q13 | Does the layer shift run only one way, with no value-to-effect operator? | LEANING, decides Q1 |
| Q14 | Does `select` return a masked view, a selection vector, or a copy? | OPEN |
| Q15 | Backend: LLVM via inkwell, Cranelift, or both? | LEANING, LLVM for release |
| Q16 | String representation, given WTF-16 on the JS target | OPEN, decides the string API permanently |
| Q17 | Is there a dense tensor type, constructed explicitly? | LEANING yes |
| Q18 | Does `.[]` on a rank-2 tensor yield rows or scalars? | LEANING rows |
| Q19 | How are nulls carried in a dense typed buffer? | LEANING, Arrow validity bitmask |
| Q20 | How are blocking operators (`sort`, `group_by`, joins) classified? | OPEN |
| Q21 | What guarantees batch size is unobservable over a batched stream? | OPEN |
| Q22 | Are dense and masked vectors distinguishable in the type? | OPEN, Q14 from the other side |

Q5 is the one that blocks building anything. Q1 and Q9 both change the two-layer section, so
they should be settled before that section is treated as stable.

### Detail

1. **Are streams first-class values, or evaluation-level multiplicity?** jq has no stream
   *value*: `Val` has no stream case, and streams exist only during evaluation. That keeps
   values finite and acyclic, and lets streams compile to loops rather than heap-allocated
   iterators. But the base-functor formulation above implies a *remainder*, which is a value.
   Possible resolution: coinductive in the type system, erased by the compiler when the stream
   provably does not escape, with an acknowledged cliff when it does.
2. **Binary operators over two multi-valued expressions.** Cartesian (jq today), zip
   (vectorized, with broadcast), or neither by default with explicit `cross` and `zip`?
3. **What symbol replaces `=`** for the product-forming assignment?
4. **Ordering guarantees over heterogeneous streams.** Subsumes the older cardinality-versus-
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
5. **The stream-lowering strategy**, which must be decided before any backend is written. Lua
   has true coroutines, JavaScript has generators, native has neither for free.
6. **Does a reconciler belong in the language** as a first-class construct, or in a library?
7. **Does `..` promise depth-first order, or only the set of nodes?** On a flat columnar layout,
   "every node at every depth" is "every element of every buffer", which is embarrassingly
   parallel. The dependent part is not the traversal but the *order*, since the flat layout is
   not in depth-first order. jq promises the order. If this language only promises the set,
   recursive descent becomes one of the cheapest operators rather than one of the most
   expensive. This is not only a performance question: a jq-derived language that is fast
   everywhere except recursive descent has a positioning problem, because `..` is one of the two
   things people reach for jq to do.
8. **Is vectorizability visible in the type system, or a silent optimization?** Reporting it
   means a second effect alongside cardinality, and a visible fast-path/slow-path distinction in
   signatures. Hiding it makes performance unpredictable in exactly the way this design is
   trying to avoid. Note the two effects are orthogonal: `select` changes cardinality and
   vectorizes fine as a mask, while `first` changes cardinality the same way and cannot
   vectorize at all.
9. **Are vectors multidimensional, with `[]` as projection?** See the TODO and response in the
   cardinality section. Unifies indexing with iteration, but disturbs the claim that there are
   exactly two layer shifters, and per-dimension cardinality only describes rectangular data
   while JSON is ragged.
10. **Is uniqueness analysis in scope?** Deciding when a projection lens can materialize instead
    of staying a view requires knowing no other reference to the source survives. That is
    linearity or uniqueness typing, the machinery deliberately avoided in Q4.
11. **How does the query/transformation split manifest?** SETTLED: it does not need to. The two
    are the same operation with the multiplicity stored in different places, so `map` and
    `select` are not different kinds of thing. `map(f)` is `[ .[] | f ]`, meaning reflect,
    apply, reify, and `[...]` absorbs whatever cardinality the argument had. See the two-layer
    section. This was the longest-running untracked thread and is recorded here so it is not
    reopened by accident.
12. **On a type mismatch, does field access error, yield null, or something third?** SETTLED:
    something third. jq conflates missing with type error, so `null.a.b.c` yields `null` while
    `1 | .a` raises. Field access desugars to a lens returning three distinguishable outcomes: a
    value, a *specific* absence, and a *specific* error. See the field-access section.
13. **Does the layer shift run only one way?** If effect multiplicity is born only from
    streaming sources and dies only into values through `[...]`, then no value-to-effect
    operator is needed, because degrading a `Vec` forgets its extent and buys nothing. LEANING
    toward yes. This decides Q1 with it, since the only thing that would break it is a value
    with genuinely unknown extent, which is what a first-class stream value would be.
14. **Does `select` return a masked view, a selection vector, or a copy?** See the section on
    whether a value-layer `select` copies. A bitmask breaks `Vec`'s constant-time indexing
    promise, a selection vector keeps it and pays memory per survivor, and either view pins its
    whole source buffer alive.
15. **Backend: LLVM via inkwell, Cranelift, or both?** Cranelift never auto-vectorizes, which
    matters because the whole vectorization argument here rests on handing a vectorizer loops it
    likes. It also emits nothing for the web, so a WebAssembly target would need a second
    unrelated backend. Cranelift wins decisively on build simplicity and compile speed, so using
    both, as rustc does, is a real option. LEANING toward LLVM for release builds.
16. **How are strings represented, given that the JavaScript target uses WTF-16?** The three
    options are WTF-16 everywhere, UTF-8 everywhere with the JavaScript-shaped API emulated, or
    designing the difference away by never exposing code-unit indexing or length. Only the third
    is cheap on both sides. It has to be decided early because it constrains the string API
    permanently.
17. **Is there a dense tensor type, constructed explicitly?** `@f32` as a narrowing constructor
    that hard-fails rather than an inference, with `reshape` attaching shape. It is also the
    second number type, a deliberate lossy exit from the `f64` commitment.
18. **Does `.[]` on a rank-2 tensor yield rows or scalars?** NumPy and APL both yield rows,
    which makes `map` rank-polymorphic and gives row sums as `map(fold(add; 0))` with no new
    syntax. Then rank-1 yields scalars and full linearization needs a separate flattening view.
19. **How are nulls carried in a dense typed buffer?** JSON has null and an `f32` buffer does
    not. NaN as a sentinel collides with genuine NaN. Arrow's separate validity bitmask solves
    it and brings zero-copy interop with Polars, DuckDB and pandas.
20. **How are blocking operators classified?** `sort`, `group_by` and joins are one value in and
    one value out, so the per-element cardinality mapping does not describe them. They need the
    whole input before producing anything and are parallelizable by other means. The
    kernel-admissibility result covers elementwise filters only, and this is the gap it leaves.

## Non-goals

- JavaScript semantic compatibility. Prototype chains, `this` binding, coercion, and array
  holes are explicitly not wanted.
- Being jq. Compatibility with a subset of jq's *semantics* is a starting point and a test
  corpus, not a constraint on the finished language.
