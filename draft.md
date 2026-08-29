# toylang design draft

**Status: exploratory. Everything here is provisional.** This is a thinking document, not a
specification. Syntax is illustrative; several core decisions are still open (see the end).

## What this is

A compiled, statically typed language derived from the jq family -- a jq dialect that keeps
jq's semantics as its reference while extending and generalizing them, aspiring to be a real
language rather than a study.

- **Data-oriented.** JSON is the native value model, not a library, and data orientation is
  the organizing principle: transformation, selection, and querying of data are how programs
  are structured, not one feature among many.
- **Compiled**, with seven backends (native/LLVM, JavaScript, Lua, jq, Go, Python, Rust) --
  kept as falsifiers of the design rather than as a compatibility promise (ADR 0002).
- **Rust-like syntax**, aiming at a version of Go's simplicity without following it into the
  absurd: where Go's refusals cost too much, ideas come from Rust instead. Enums are the
  first such import.
- **The ambition is general-purpose** -- eventually HTTP servers, GUIs, web frontends, shell
  scripts -- but through a deliberate sequence. The beachhead is CLI data transformation,
  where the dogfood test is concrete: it replaces jq in its author's own shell. Shell
  scripting is the first area to expand into after that. Result-set-oriented tooling (an
  editor whose buffer is a query result) stays on the long horizon, and nearer-term design
  decisions must not foreclose it.

Aspiring to be real puts weight on claims that a study could leave as prose: the performance
thesis (columnar, vectorized, faster than jq's boxed-iterator-per-step ceiling) is a
commitment that will eventually owe benchmarks, and positioning worries like
[recursive descent's cost](#q7-does--promise-depth-first-order-or-only-the-set-of-nodes) are
real product concerns, not rhetorical ones.

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

**Chosen: the collapse, with a spelling.** Multiplicity in the effect layer is
evaluation-level, and `Stream<T>` is its type -- the type of an expression that yields its
entries as evaluation proceeds, not the type of a stream object. The cardinality table below
and this section were describing the same thing from two sides, which is why earlier versions
of this document read as contradicting each other here. The alternative, streams as
first-class values, would have meant three things to track (`Vec`, a `Stream` value, and
effect multiplicity) and a held value of genuinely unknown extent. See
[the decision](#decided-stream-is-the-effect-layer-typed) for the full rules.

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

It answers [the streams question](#q1-streams-first-class-values-or-evaluation-level-multiplicity). If effect multiplicity is only ever born from I/O and only ever collapses into
values, there is no need for a `Stream<T>` *type* at all. `Stream` becomes an effect annotation
on an expression rather than a type constructor, which is the evaluation-level answer arriving
for a fourth time.

It simplifies [the recursive-descent ordering question](#q7-does--promise-depth-first-order-or-only-the-set-of-nodes). If `..` is value-layer, producing a collection of nodes, then whether it
promises depth-first order is an ordinary question about how a value is ordered, rather than a
question about evaluation strategy.

The case that would break it: any value with genuinely unknown extent. That is exactly what a
first-class `Stream` value would be, so [the streams question](#q1-streams-first-class-values-or-evaluation-level-multiplicity) and this proposal stand or fall together.

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
collides with principle 2 and with [the vectorizability question](#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization): the type would say "view", and the runtime would
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

One connection: adopting projection semantics pulls [the binary-operator question](#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit) (cartesian vs. zip) toward
broadcast, because multidimensional projection and elementwise broadcast are the same tradition.
[That question](#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit) and this TODO may be one question.

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

Three distinguishable outcomes: a value, a *specific* absence, a *specific* error. See
[Pattern matching is decoding](#pattern-matching-is-decoding) for how these three outcomes extend into a full decode syntax.

## Pattern matching is decoding

The matcher surface's first cut is decided; see
[the arms decision](#decided-match-arms-compose-with-or-and-a-guard-chain-may-be-honestly-partial),
which grew out of FizzBuzz as the case study and retired `//` in favor of `or`.

TODO (user): Pattern matching in this language might not need dedicated syntax. Instead, an
"inline mapping" reuses the same shape a zod-style parser already has: a matcher on one side,
the unpacked value on the other. `int(.x) -> .x * 2` matches only if the subject is an integer,
and fails otherwise; a list of these, `int(.x) -> .x * 2, any() -> .`, never fails, because
`any()` matches anything. `.x` (with the dot) reads a field off the matched value; a bare `x`
would instead be a fresh variable, so the dot has to be mandatory on the matching side to keep
the two apart. That raises the question of a short, standard way to refer to "the value that was
matched" rather than repeating its shape on both sides. If matching without transforming --
`int(.) -> .` -- is just running the decode, this is a codec system in the zod/serde sense, and
the natural next question is whether there is an inverse operator that encodes as well.

RESPONSE (agent). Four things carry over from further up the document rather than being new.

`.x` on the matching side is the right call, and it is not a new rule. `.` already means "the
implicit subject" everywhere else -- `map(.name)`, `select(.age >= 18)` -- so requiring `.x` to
mean "read field x off the subject" and leaving bare `x` to mean "bind a fresh name" is the same
convention showing up in a new spot, not a special case invented for matching.

`.` as the shorthand for "the matched value" is likewise already spoken for. `json.name!` in
[the field-access section](#field-access-is-a-lens) is the existing idiom for "give me the value or propagate the failure," and
`int(.) -> .` is the same shape: run the decode, keep what comes out unchanged. No new symbol is
needed, since `.` was already "the current subject" before matching existed.

The ordered arm list, `int(x) -> x*2, any() -> .`, should not actually be spelled with `,`. This
document's `,` already has a meaning, "branch wider, explore every alternative"
(see [Query is search](#query-is-search)), and an ordered, first-match-wins list is the opposite of that: once `int(x)`
matches, `any()` is never even considered. That is exactly what `//` already means, "alternatives,
left to right," so the arm list is a sugared `//` chain, and spelling it with `,` would make `,`
mean two different things depending on where it appears.

The inverse operator is real but it runs into a gap already on record. Decoding `Json -> T` is
free: it is the same move as `as_tensor : Vec<Vec<Num>> -> Result<Tensor<Num, [n, m]>, ShapeError>`
in [the tensor section](#a-dense-tensor-type), generalized to arbitrary shapes. Encoding `T -> Json`
needs to *build* a `Json` value from a `T`, and object construction does not exist yet -- see
[a type you can declare but cannot build](research-log/a-type-you-can-declare-but-cannot-build.md). So this design would replace the decode half of
serde today and the encode half only once that gap closes.

TODO (user): Pattern matching and codecs might both be an extension of boolean algebra, which
argues for Python-style `and` / `or` / `not` rather than `&&` / `||` / `!`, to keep them visually
apart from `|` as pipe. That raises a fork: does `Result` itself gain boolean-algebra traits, or
is there a separate result-shaped type that carries the algebra instead, if for some reason
`Result` should stay out of it? Convenient unpacking would also need a short way to turn an error
into a runtime failure so it can feed back into the matching syntax. Codecs might double as type
constructors -- `type Foo` could have a same-named codec `foo`, possibly enforced by casing at
compile time. The character standing for "the matched value" might as well be `.`, partly because
it would shadow the un-matched, wider value at the type level, so a branch body cannot reach past
what it actually matched. And codecs presumably need to be overloaded: serializing to a string is
a different operation from unpacking an already-typed enum.

RESPONSE (agent). One of these should be adopted outright, one should be split into a distinct
type, one connects to an existing open question rather than being new, and one should be deferred
rather than enforced.

Adopt `and` / `or` / `not`. `|` is already claimed by pipe; `||` sitting next to it is exactly the
readability collision Python's split avoids, and for the same reason.

Put the algebra on a new `Matcher<T>`, not on `Result<T, E>`. Making `Result` itself
boolean-truthy would let a value read as "successful" while still carrying an unread error, which
is the missing/error conflation [Q12](#q12-on-a-type-mismatch-does-field-access-error-yield-null-or-something-third) was written to rule out. A matcher's `and` / `or` combine the yes-or-no
question of whether something matches; *running* a matcher against a value is the separate step
that produces the `Result`. REJECTED: giving `Result` the algebra directly.

The algebra is not new, either. [The regular-expressions-over-types idea](#q4-can-the-type-express-ordering-over-heterogeneous-streams) already proposes `Alt<A,B>`, `Seq`,
and `Star` as the primitives for describing a sequence of types. `int() or str()` is `Alt`, a
record pattern's fields in sequence is `Seq`, and `any()*`-style repetition over a `Vec` is
`Star`. This is that algebra's value-level instance rather than a second one.

One distinction the syntax hides and the compiler must not: matching a value whose type is
already closed -- an enum you already have an instance of -- is total, checkable for
exhaustiveness at compile time, no `Result` anywhere. Matching untyped `Json` is partial, since a
shape nobody wrote a case for is a live possibility, so it needs `any()` or a `Result`. The two
compile to different things behind one shared arm syntax.

`.` shadowing the wider value inside a matched arm is correct, and it is [the same rule the document
already has](#annotation-rule-named-functions-declare-lambdas-never-do) for block-scoped `.`:
`map(.name)` rebinds `.` to each element already, so a matched arm rebinding `.` to the narrowed
type is that rule applying once more, not a new one. Inside `int(.) -> ...`, `.` has type `Int`,
not `Json`, and the wider `Json` is not reachable by name.

Convenient unwrap-to-error already has its syntax: `.name!`, from [the field-access section](#field-access-is-a-lens),
already turns absence or error into a propagated failure, and composes directly with a matcher --
`int(json.name!) -> ...` decodes only after the field access has already committed to failing
loudly rather than quietly.

Adopt the naming convention, defer the enforcement. A type and its constructor sharing one name,
resolved by which namespace looks it up, is precedented (Haskell's `data Foo = Foo Int`, Scala's
companion objects). But *requiring* every `type Foo` to have a working `foo` codec means every
declared type needs a constructor, which runs straight into the still-open gap that records can
be declared but not built. Style convention now; compile-time enforcement only after that gap is
closed.

Overloading is real, and it is the total/partial split again rather than a third axis. `Json -> T`
decode, `T -> Json` encode, and `Str -> T` parse are three instances of one trait family, picked
by which types the codec sits between -- the same way [`Field<K>`](#field-access-is-a-lens) already picks an
implementation by `K`. Encode is still blocked on the same construction gap named above.

TODO (user): Given a large structure, the matcher should be able to name only the parts it cares
about without spelling out the rest of the shape. Separately, matching something at an unknown
depth -- where the path to it either is not known ahead of time or is inconvenient to write out --
seems to call for some kind of aliasing.

RESPONSE (agent). The first is free on one side and needs one borrowed marker on the other; the
second already exists, and "aliasing" turns out to name two different features of very different
size, only one of which this idea actually needs.

Naming a subset of fields against `Json` costs nothing extra, because `Json`'s fields were already
`Opt<Json>` -- structurally open -- before matching existed. `{name: str(.name), age: int(.age)}`
builds exactly a two-field record and silently ignores every other key in the source object,
because the pattern never claimed those other keys mattered. No rest-marker is needed on this
side; openness already said "there may be more."

Naming a subset of fields against a value of an already-closed type is a different claim, and does
need a marker, borrowed from Rust: `User { name, .. } -> ...`. Without `..`, leaving fields out of
a match against a closed type should be a compile error by default, the same exhaustiveness
guarantee an enum match gets. `..` is what turns "I forgot a field" into "I meant to ignore it."

Path-independent matching is already expressible with what the document has. [`..`](#query-is-search) is full
depth-first traversal today; composing it with a matcher as the predicate is the whole feature:

```
tree.. | int(.) -> .        # every Int anywhere in the tree, whatever its path
```

No new operator, because the entire point of `..` was already "I don't want to name the path."

This does raise the stakes on [whether `..` promises order](#q7-does--promise-depth-first-order-or-only-the-set-of-nodes). Before, the order it walks in was
mostly a performance question. Once `..` is the mechanism for "find this shape somewhere," two
runs disagreeing on order is a correctness problem for whatever consumes the result, not only a
speed one. Worth folding into that question's detail rather than opening a new one.

"Aliasing" a submatch so later code can refer to it by name is also already in the document, just
not yet generalized. `("red","blue") as $c` in [the record-forming-update section](#undecided-what-to-call-the-record-forming-update) already binds a
whole expression's result to a name; extending `as` to a matched submatch is the same binding,
applied one level deeper:

```
{x: int(.x), y: int(.y)} as point -> point.x + point.y
```

This is an as-pattern (Rust and Haskell both have one), not new machinery.

What "aliasing" does not cover, and should not be assumed to, is unifying two *separate* deep
matches against each other -- finding a node `A` somewhere and a node `B` elsewhere such that `B`
refers to `A`. [Query is search](#query-is-search) already grounds this document's nondeterminism in Prolog's
vocabulary, so the connection is real, but that feature is full logic-variable unification with
backtracking over bindings, which is a much larger commitment than binding one submatch to one
name. Tracked below as its own open question rather than folded into this one.

#### The constructor should be free, and that reverses a dependency claimed above

TODO (user): Not every type needs a JSON codec, but every way of declaring a new type -- struct,
enum -- could auto-generate at minimum the plain constructor that already exists implicitly in
the declaration: the function from the declared field types to a value of the declared type. If,
in addition, every built-in scalar and every built-in generic (`Vec<T>`, `Opt<T>`, and so on)
carries a Json encode/decode pair, then for the common case -- a type built purely out of things
that already have one -- the Json codec composes for free too, recursively. Possibly even a JSON
Schema document falls out the same way.

RESPONSE (agent). Correct, and it overturns the "defer enforcement" call two entries up. That
call conflated two different artifacts under one name.

The bare constructor -- `Foo(name: Str, age: Int) -> Foo` from a declaration of `type Foo =
{name: Str, age: Int}` -- is not a codec at all. It takes already-typed values and is total: it
cannot fail, because assembling already-checked fields into a record has no failure mode. This
is what Rust, Haskell, and every ML-family language already give a `struct`/`data` declaration
for free, and toylang not having it is a gap in the declaration syntax, not something waiting on
a design decision elsewhere. There is no reason to defer this one: generate it unconditionally
for every `struct` and `enum` declaration.

That also reverses the dependency claimed two entries up: the naming convention was said to be
blocked on object construction existing first. It runs the other way. The free constructor
generated by the declaration *is* object construction, so there is no separate curly-brace
literal to design; `Foo(name: "ada", age: 36)` already is one.
[The type-you-can-declare-but-cannot-build gap](research-log/a-type-you-can-declare-but-cannot-build.md) closes as a side effect of giving declarations a
constructor, rather than needing to be closed before declarations can have one. It also folds
under [checked-only forms are a class, not a lambda rule](research-log/checked-only-forms-are-a-class-not-a-lambda-rule.md): a `{...}` literal checked against a
declared record type is a fourth instance of the same class already identified there (alongside
`input`, empty `[]`, and lambdas), and the free constructor is what gives that instance a name
rather than leaving it anonymous.

The `Json -> T` codec is a second, separate artifact, and "free" is conditional rather than
unconditional the way the constructor is. It is free exactly when every field's own type already
has a codec -- true by definition for the built-ins, and true recursively for any type built only
out of things that already have one, the same structural composition [`Field<K>`](#field-access-is-a-lens) already uses
per field. Most declared types are exactly this case, so deriving the pair mechanically, rather
than requiring it be hand-written, is the right default. Precedent: Rust's
`#[derive(Serialize, Deserialize)]`, Haskell's generic `aeson` deriving, Scala's circe
derivation.

One place this does not fall out for free: an enum needs a **discriminant convention**, some
agreed way for the `Json` to say which variant it is (`{"kind": "circle", "r": 1}` versus
`{"circle": {"r": 1}}` versus matching on shape alone). Struct fields compose without asking this
question; variants cannot avoid it. Tracked as [Q29](#q29-what-is-the-default-discriminant-convention-for-a-derived-enum-codec).

A JSON Schema document is a third projection of the same structural information, and it is
cheaper than the codec rather than harder: it describes the shape without running any decode
logic, so it falls out of whatever internal representation the derived matcher already builds --
the same representation [the regular-expressions-over-types idea](#q4-can-the-type-express-ordering-over-heterogeneous-streams) assigns each type (`Alt` for a
discriminated union, `Seq` for a record's fields).

TODO (user): Types, constructors and codecs are probably one and the same thing, with the
built-in generic types making up a generic codec-and-data-structure compositor.

RESPONSE (agent). The composition half is already settled twice over, above and in the next
section. What is new here is the *identity* claim, and that part needs one correction, because
the entry two up deliberately separated the same three things.

Right about the mechanism. One declaration yields a structure, a constructor, a decoder, an
encoder and a schema, and all of them are derived by the same recursion over the same base
functors: a record sequences, a sum chooses, `Vec<T>` repeats, `Opt<T>` makes optional. Nothing
is written per artifact; each is a projection of one structural description. That is the useful
content of "one and the same thing" and it is worth stating as bluntly as the TODO does.

Wrong about the artifact, and the difference is total versus partial. The constructor takes
already-typed values and cannot fail: assembling checked fields into a record has no failure
mode. The decoder takes untyped input and can fail on every field. They cannot be the same
function because one has a failure mode and the other does not, which is also why the
constructor is unconditionally free and the codec is free only when every field's own type
already has one.

So: one description, several projections, and the projections differ in whether they can fail.

A prediction from that entry has half come true from the other direction. It said the free
constructor *is* object construction, so no separate brace literal needs designing. The brace
literal got built first, and with the parens rule of
[the record literal section](#decided-records-can-be-built-and-a-record-is-how-several-arguments-travel),
`Foo {name: "ada", age: 36}` is the shape that entry wanted from `Foo(name: "ada", age: 36)`,
differing only in the bracket. The two converge rather than one replacing the other.

## One combinator algebra for trees, strings, and streams

TODO (user): The built-in generic types might inherently double as parser-combinator primitives
-- `Vec<T>` as "many `T`", `Opt<T>` as "optional `T`", a sum type as choice, a record/record as
sequencing. Separately, and perhaps as an application of the same idea, strings could get a
pattern language that reads more like a URL route pattern than a regular expression: named,
typed capture groups, easier than regex for the common case, but able to fall back to full regex
power. Sketch offered and explicitly flagged as a bad first attempt at syntax: `b:foo{mul("a")}b`
in place of `ba*b`, and `b:foo:b` in place of `b.*b`.

RESPONSE (agent). The first half already has a name and thirty years of literature: this is
parser combinators, built on exactly the base functors already in the document. `Seq` is
`and_then`/record, `Alt` is `choice`/sum, `Star` is `many`, `Opt` is `optional`. Hutton and
Meijer's original combinator paper and Wadler's "How to Replace Failure by a List of Successes"
derive a parser library from precisely this shape; parsing with derivatives (Might, Darais, and
Spiewak) is the same idea again, and it is also what [the regex-over-types entry](#q4-can-the-type-express-ordering-over-heterogeneous-streams) is already
reaching for with "unpacking one item is the derivative of the pattern." Naming it does real
work: this design does not have to invent parsing theory, only decide which known combinator set
to adopt.

One thing the name does not give away for free: a `Json` tree is already fully parsed, so
recursing through it needs no backtracking and no notion of position, while a string or byte
stream needs an actual parsing engine underneath, because "does `a*` match here" can require
trying more than one length before the surrounding pattern succeeds. So this is one algebra with
at least two implementations, the same shape [`Field<K>`](#field-access-is-a-lens) already has for indexable versus
iterable receivers. What would need stating, and is not yet, is the law the two implementations
have to share to count as the same trait -- in the spirit of
[the batch-invariance law](#the-admissible-input-set-and-where-batching-comes-from), but for "matches the same shape" rather than "commutes with
reification."

The string pattern language is a specialization of exactly this algebra, not a separate feature.
Spelling repetition as a named combinator call rather than a metacharacter (`mul("a")` instead of
`a*`) trades density for not needing a second syntax to learn, the same trade this document
already made for `and`/`or`/`not` over `&&`/`||`. Named, typed captures need no new mechanism
either: a capture group decoding to an `Int` is `int(.)` from [Pattern matching is decoding](#pattern-matching-is-decoding) applied to a
captured substring instead of a `Json` field, so "more specific than string" is the existing
codec syntax, not a new one.

One consequence is already decided without having been meant to be. [The arm-list section](#pattern-matching-is-decoding) settled
on `//`'s left-to-right, first-match-wins semantics for alternation rather than `,`'s
explore-all semantics. That is exactly PEG's defining feature over classical regex/CFG
alternation: ordered choice, no ambiguity, no need to explore every branch. It is compatible with
PCRE/Perl-style backtracking regex, which is also priority-ordered, but not with POSIX
leftmost-longest regex (`grep -E`, `awk`), which is a genuinely different alternation semantics.
So "extends to regular expressions" should be read as "extends to PCRE-flavored regex"
specifically -- a real, load-bearing consequence of a decision already made, not a detail to
leave implicit.

Closest existing prior art for the surface syntax, worth reading before inventing one from
scratch: Swift's `Regex` builder (named, typed captures composed via a result-builder DSL, fully
interoperable with a real regex engine) is close to what the TODO describes, and path templating
in Express's `path-to-regexp` and Rails routes is close to the "URL pattern" framing, including
their convention of embedding a raw regex inside a named segment for cases the friendly syntax
cannot express (`:id(\d+)`) -- the same "friendly by default, escape hatch to full power" shape
being asked for here.

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

[The vectorizability question](#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization) was argued on the grounds that cardinality and vectorizability are *orthogonal*: `select`
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

The split between these does not need to reach the surface. It can exist **only at the type
level**: one semantics stated as a trait, with a different implementation when the receiver is a
`Vec` and when it is a `Stream<Vec<T>>`. The same move as `__project__` having one impl for
indexable things and another for iterable ones. A user writes `map(f)` once and the compiler
picks the implementation from the type.

That is worth more than the ergonomics, because of what it does to batch invariance.

**The law is that the operation commutes with reification.**

```
op(f) . reify   ==   reify . op(f)
```

Applying an operation and then collecting gives the same answer as collecting and then applying.
An operation satisfying that cannot observe batching, because reifying at any point in the
pipeline yields the same result. So batch invariance stops being a rule to police separately and
becomes the trait's law, which both implementations have to satisfy in order to be
implementations of the same thing at all. A trait without a stated law is only overloading, and
this is the law.

**[The blocking-operator question](#q20-how-are-blocking-operators-sort-groupby-joins-classified) then answers itself.** The blocking operators are exactly those with no lawful stream
implementation. Sorting each batch does not sort the stream, so `sort` cannot satisfy the law
batch-locally. Its options are to have no stream impl, which is a compile error and honest, or
to buffer the whole stream, which silently defeats streaming. `group_by` and joins are the same
shape. So "blocking operator" is not a separate category that needed inventing; it is the name
for a trait with a missing instance.

`first` by contrast does have both, which is consistent with the resolution above: over a `Vec`
it is a minimum index over a mask, over a `Stream` it short-circuits, and both give the same
answer, so the law holds.

**Cost still differs where the law holds, and that is fine.** `map` over a `Vec` is one launch;
over `Stream<Vec<T>>` it is one per batch. Same result, different performance profile. Symmetry
survives because the *type* still says which implementation was selected, so the cost difference
is visible in the signature rather than hidden in the dispatch.

### The primitive set cannot be fold and recursion

Functional languages usually build everything on a higher-order function plus recursion, and
conventionally that function is `fold`. Every list operation is a catamorphism, which is elegant
and completely sequential. Anything defined that way inherits the sequentiality of its
definition, so a standard library written over `fold` cannot be vectorized no matter what the
backend does. General recursion has the same problem for the same reason.

So the basis has to be different. The established parallel basis, from Blelloch's work on scans
as primitive parallel operations and used since by NESL and Futhark, is small:

```
map          elementwise                         depth 1
scan         prefix sum over an associative op   depth log n
reduce       associative op                      depth log n
gather       permutation by an index vector      depth 1
scatter      inverse permutation                 depth 1
             plus the segmented form of each
```

Five operations and their segmented variants. Everything else is built from them: compaction is
a scan followed by a scatter, radix sort is a sequence of scans, and partitioning is compaction
by a predicate. Notice that `fold` with an arbitrary operator is *not* in the set, while `reduce`
with an associative one is. That is the same line the `reduce`/`fold` split already draws, now
determining what the standard library may be defined over rather than only how one operator
compiles.

The precise characterization of which folds belong is the **third homomorphism theorem**: a
function expressible both as a left fold and as a right fold can be computed by an associative
divide-and-conquer. That is exactly the condition an operator has to meet to earn a place here.

Recursion splits the same way. **General recursion cannot be flattened**, but *structural*
recursion over a finite structure can, by the flattening transform this document already relies
on: recursion over a tree becomes segmented operations over a flat buffer plus offsets. Which is
also why the segment descriptors are load-bearing rather than an implementation detail.

Two consequences worth stating plainly.

**The standard library should be defined over the parallel basis, with `fold` and general
recursion as leaves rather than as the root.** If they sit at the root, every derived operation
inherits a sequential definition and the vectorized path can only ever be a special case that
the compiler recovers by accident.

**The primitive set and the lawful-stream-instance predicate are the same boundary again.**
Operations definable from the basis have stream implementations that satisfy the commuting law;
operations needing general recursion or a non-associative fold do not. This is the third time a
single distinction has done duty for what looked like separate questions, which is either the
design cohering or a sign that the same idea keeps being renamed.

The honest cost: expressing a computation as a scan is genuinely less obvious than expressing it
as a fold. `sum` as a fold is immediate; as a scan taking the last element it is indirect. Array
languages pay this and it is a real ergonomic tax, not a free win.

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
to be distinguishable in the type, since they have different launch preconditions. This is [the select-result question](#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy)
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

TODO (user): stdin and stdout should not require solving generic effect types, because they are
always string streams and so have no choice of element type. At worst the end of input shows up
at the type level, and a `ClosableStream<Str>` primitive, working the way `Opt<>` does, would be
the most trivial effect type: one that promises it is potentially not infinite in time.

RESPONSE (agent). The pragmatic move is right and the bit being asked for already exists, but it
is named on the wrong side, and the proposal cuts against a leaning that has four arguments
behind it. Those are three separate things.

**The bit already exists.** In the base-functor table above, `Stream<T> = nuX. 1 + T*X`, and the
`1 +` is exactly closability: it is the case where the stream stops. So `ClosableStream<T>` is
`Stream<T>` as already defined. What has no name is the *other* one, `nuX. T*X`, which never
stops. If a name is wanted for the distinction, it should attach there, because that is the case
a reduction over the stream can never produce a value for.

**The pragmatic move is right.** stdin and stdout are byte streams with no element type to
choose, so the machinery a general `Stream<T>` needs is not needed to build them. That is a real
saving and it is the difference between starting now and waiting on
[Q1](#q1-streams-first-class-values-or-evaluation-level-multiplicity).

**The tension.** Q1 leans evaluation-level, meaning there is no `Stream<T>` *type* at all and
`Stream` is an effect annotation on an expression. A `ClosableStream<Str>` primitive is a stream
type, so taken generally it reverses that.

The sidestep that makes it a starting point rather than a reversal: one concrete opaque built-in
is not a type constructor. A file handle is a value in languages that have no first-class
streams, and `ClosableStream<Str>` can be that, monomorphic and un-parameterisable, without
committing to `Stream<T>` being spellable over an arbitrary `T`. That is a middle position worth
naming deliberately, because arriving at it by accident would look exactly like having answered
Q1 without noticing.

What to watch, if this is built: whatever is written against the concrete primitive has to
survive `Stream` later becoming an annotation rather than a type. The safe version keeps stdin
and stdout as opaque handles that only a small set of operations touch, so that the operations
are what generalise and the type does not have to.

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

### UNDECIDED: what to call the record-forming update

In jq, `=` is not assignment. Its right-hand side is an ordinary expression, so if it yields
several values, the whole update yields several results:

```
{} | .a = (1,2)        # -> {a:1}, {a:2}      TWO objects, not one object with two values
```

That is genuinely useful. It gives config-matrix expansion, variant generation, and
property-test input enumeration for free. The problem is purely that `=` *looks* like mutation
while behaving like a record, and the multiplicity is invisible at the call site.

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
a record is wanted, it is written out:

```
db.color = "red"                        # ok
db.color = ("red", "blue")              # ERROR: expected One<Str>, found 2 values
("red","blue") as $c | db.color = $c    # explicit; jq already supports this and it reads better
```

**C. Two distinct operators.** `=` for the single-valued case, and a visually distinct one for
the deliberate record, such as `.color =* ("red","blue")` or `.color each= (...)`. Keeps both
without either being silent, at the cost of more surface.

**D. Drop `=` entirely and keep only `|=`.** All updates go through the update operator, and
records come from an explicit `cross` or `for` construct. Smallest core, largest departure.

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

## PROPOSAL: every dimension gets a spec

This replaces an earlier proposal that a projection is its own type. That one was built on the
idea that `[]` licenses a lifting, which needed a `Proj<T>` to carry the licence. The lifting
framing turned out to be wrong, and with it the type.

### Where it came from

Prototype 1 let field access distribute over a `Vec` by itself, so `db.users.name` returned every
name with no `[]` anywhere, and `[]` had nothing left to do. Running the same cases through jq
showed why the operator went inert:
[jq's item-wise access is the effect layer wearing brackets](research-log/jqs-item-wise-access-is-the-effect-layer-wearing-brackets.md). `[.[][1]]` applies its
second bracket per element while `.[1][2]` applies the same token to the container, and what
separates them is that a stream came first. jq's `[]` is not the item-wise operator; it is the
usual way into the layer where everything already is. Remove the layer and nothing is left.

### The proposal

A type has an ordered list of **dimensions**, fixed by the type. An access says one thing about
each of them -- a **spec** -- and may then select a **field**:

```
value[spec][spec]...field
```

Three specs. **Keep**, written `[]`, leaves a dimension at full extent. **Narrow**, such as a
mask, reduces it. **Collapse**, such as an index, removes it.

`db.users.name` is an error because dimension 0 was never given a spec. `db.users[].name` gives
it one. That is the crossing being written down, and it is required by the grammar rather than by
a rule about lifting.

### What follows without further stipulation

**Nesting needs no rule.** One spec per dimension, so `db.groups[].members[].name` opens two and
`db.groups[].members.name` does not typecheck.

**The `Vec` and `Stream` promise falls out.** Keep and narrow are streamable, since neither has to
consume anything to know what it did. Collapse is not: finding entry three means passing the
first three, so on a stream it destroys what it passed. The difference between the two collection
types is the difference between the specs they admit.

**Rectangularity becomes a refinement rather than a gate.** Collapsing an inner dimension of
ragged data is perfectly meaningful, and jq does it: `[.[][1]]` on `[[1,2],[3]]` yields
`[2,null]`. What rectangularity buys is that there is no hole. So it does not decide whether the
operation exists, only whether the result is `Opt`.

**There is one access model, not two.** A tensor is not a second scheme with its own syntax; it is
the same scheme over a type whose extents happen to be uniform.

**A record is not a dimension.** Its field names are type-level, so iterating it would flatten
them into positional order and lose them, which is the erasure principle 1 forbids. `to_entries`
is the written-down version of that crossing, not a workaround for a missing feature. This holds
whether or not the fields share a type, so it is not a question of finding a common cell type.

### Consequences to accept

**`Map<K,V>` becomes a distinct type.** A record's keys are known to the compiler; a map's are
known only to the program. Collapsing them is what forces jq to treat an object as a struct and a
dictionary at once, which is where its own `.[]` ambiguity comes from. Accepted.

**Five deliberate divergences from jq**, all measured rather than assumed:

| | jq | here |
|---|---|---|
| `.[]` on an object | iterates values | error; use `to_entries` |
| `.[]` at top level | many outputs | one value; there is no stream |
| `.[9]` out of range | `null` | `Opt` |
| `null \| .[0]` | `null` | error |
| `[.[] + .[]]` | cartesian, `[2,3,3,4]` | not expressible by accident |

### Settled

**One bracket per dimension**, left to right: `v[3][]`, not `v[3, ]`. This is already jq's
spelling, and it composes with nesting, which is the general case here; a shape-first
comma-separated form can be added later as sugar over it if rank-3 tensors turn out to be common.

**Negative indices are a spec.** `[-1]` counts from the end. It collapses, so it was already not
streamable, and out of range yields `Opt` exactly as a positive index does.

Built, and the ragged case comes out identical to jq: `[[1,2],[3]][][1]` is `[2,null]` on all
three backends, which is `[.[][1]]` there. Keeping one dimension and collapsing the next is the
same query, arrived at from dimensions rather than from streams.

**A `Str` has no dimension.** It is a scalar, so no spec applies to it and `"abc"[0]` is an
error. jq is incoherent on this, allowing `"abc"[0:2]` while rejecting `"abc"[0]`; a string is
one value here, and reaching into it is a library operation rather than an access.

### Unwrapping

`!` insists a value is there and stops the program if it is not, which is the spelling this
document already sketched as `json.name!`. It brings the first abort into the language: every
backend now has a way to refuse, and what has to agree across them is that they all refuse rather
than what each says while refusing.

The type is what decides whether output is raw, so unwrapping changes it. `["ada","bo"][0]` is
`Opt<Str>` and prints `"ada"`; `["ada","bo"][0]!` is `Str` and prints `ada`.

`Opt` now has a spelling in the type syntax: a function can declare one as a parameter or
return type, which is what lets it hand an absence back rather than being forced to insist.
It nests the same way `Vec` and a record field already did -- `Opt<Opt<T>>` typechecks, since
nothing singled out one level before this either -- and the stream containment ban extends to
it: `Opt<Stream<T>>` is refused, matching `Vec<Stream<T>>` and a `Stream<T>` record field.

### Still open, and none of it blocks building this

Whether records are open or closed, which the input rule already prejudges by ignoring
undeclared fields. Whether a view and an owned buffer differ in the type. Whether there are
union types at all.

## DECIDED: Int is 32 bits and wraps

Settled by measurement rather than by preference, after three recommendations that each got
reversed by a fact. The rule that ended it is in
[each target constrains the design differently](research-log/each-target-constrains-the-design-differently.md): a target's speed constrains the design
only if that target is meant to be fast, while every target's correctness constrains it always.

### The decision

`Int` is a signed 32-bit integer, and arithmetic wraps. Free on native and on Lua 5.4, both of
which have hardware integers. On node it is `|0`, which disappears once V8 has type feedback, and
which keeps every value inside the Smi range where V8 is fast. On jq it is emulated exactly and
slowly, which under the rule costs nothing.

### Why not wider

53 bits looks free on paper, being exactly representable in a double, and is worse on node than
32 is. **V8's fast path is the Smi, a 32-bit integer**, so a value outside that range becomes a
heap number. That is a representation change rather than an extra instruction, and no amount of
arithmetic cleverness recovers it.

64 bits is worse again: addition needs an overflow intrinsic rather than a compare, jq cannot
represent it for arithmetic at all, and node would need `BigInt`.

### Why wrapping rather than trapping

Doubles do not wrap, so on node and jq a wrap costs a fixup that a check does not. But trapping
is a branch, and a branch is a side effect, which is what stops a reduction from vectorising. For
a language whose performance argument is columnar and vectorised, that is the wrong thing to
spend.

Verified rather than assumed: wrapping 32-bit multiplication has no direct spelling in jq, since
the true product needs 62 bits and a double holds 53. Splitting into 16-bit halves reproduces it
exactly, and agrees with C and with `Math.imul` on the cases that usually break an emulation,
including `-2147483648 * -1` and `46341 * 46341`.

### What this costs, and what pays it back later

A millisecond timestamp is 1.8e12 and does not fit; the ceiling is 2.1e9. So `{ts: Int}` will
typecheck and then be rejected by the input validator on real data. That is a loud failure with a
one-word fix rather than a silent corruption, which is the trade being accepted.

A second integer type is what fixes it, and the case it serves is narrower than it looks:
identifiers and timestamps are *carried*, not computed. So the portability cliff is in arithmetic
on such values, not in holding them.

**Corrected by measurement.** This section previously claimed every backend carries a 64-bit
integer exactly, jq included. Five of the six do, and jq is indeed one of them, since it
preserves a number's original text when it passes through unmodified. JavaScript does not: at
2^53 + 1 it is off by one, and at `i64::MAX` it is off by 193. **The portable carrying width is
53 bits, not 64, and it is JavaScript that sets the ceiling.**

That narrows the choice rather than settling it. A millisecond timestamp is 1.8e12 and is exact
on all six, so 53 bits covers the case that motivated this. Snowflake ids and `bigint` primary
keys are past 2^53 and are not covered, and reaching them means JavaScript uses `BigInt` and
carries a second numeric representation. Which of those two the second type is for is still
open.

A timestamp type is a separate question again, and a better answer than an integer either way.

### The operators, and the one way they fail

`+ - * / %`, with `* / %` binding tighter than `+ -` and both tighter than comparison. Unary
minus is a prefix operator, which is what forced the negative-literal lexing to go: `a -1` would
otherwise not be `a - 1`.

`+` is the only operator whose meaning depends on its operands, adding two `Int` and
concatenating two `Str`. Nothing is coerced, so `1 + "a"` is an error. Comparison already
dispatched on operand type, so this is the second such operator rather than the first.

Division and remainder **truncate**, so `a == (a/b)*b + a%b` holds and `-7 % 3` is `-1`. Chosen
because three of the four backends do it natively, and because Lua's `math.fmod` is already
truncated, which made it free on the fourth as well. Floored was the alternative and cost extra
on three of four.

**A zero divisor is the only way arithmetic can fail.** Everything else wraps, including
`MIN / -1`, which is `MIN` by the same rule that makes `MIN * -1` be `MIN`. Trapping there was
the alternative and it is free on x86, where the hardware raises the same fault as for a zero
divisor -- but it would mean multiplication and division disagreeing about what overflow does,
which is a footnote that never goes away. One rule for arithmetic and one way to fail was worth
more than the branch it costs.

Natively that branch does not exist either: computing in i64 and narrowing to 32 bits makes
`MIN / -1` produce 2^31 and wrap back, so the case that costs C and Rust a check costs nothing
here.

Every wrapping edge is checked against C in the corpus, including `-2147483648 / -1`,
`-2147483648 % -1`, `-(-2147483648)` and `46341 * 46341`.

### The literal was where the rule leaked, and a fifth backend found it

An `Int` literal wider than 32 bits used to be accepted, and all four backends printed it back
unchanged. They agreed, so the agreement harness was quiet. They agreed by coincidence rather
than by rule: each held the literal in its own wider representation and only wrapped once an
operator touched it. `9999999999` was an `Int` that had never met the type.

Go would not go along. Its constant arithmetic is exact and unbounded, and a typed constant that
does not fit is a **compile** error rather than a wrapped value, so `int32(2147483647) + int32(1)`
does not build at all. That is the reverse of every other target, where the wrap is what is free
and the check is what costs.

Two things came out of it.

The rule is now stated where the value enters: **an `Int` literal must fit in 32 bits**, and a
minus directly on a literal is part of the literal, so `-2147483648` is writable and
`-2147483649` is not. This is Rust's rule, and it is the reason unary minus was left as a prefix
operator rather than folded back into the lexer -- `a -1` still has to mean `a - 1`.

The Go backend passes every literal through `func tlInt(n int32) int32 { return n }`, which makes
the expression non-constant so the wrap happens at runtime where it is defined. Go inlines the
call away, so the rule costs nothing at either end: `int32` wraps natively on `+`, `-` and `*`,
truncates on `/` and `%`, and defines `MIN / -1` as `MIN` -- the only target that agrees with
every arithmetic decision here without being made to.

### Six backends, and why there are six

`lua`, `js`, `llvm`, `jq`, `go` and `py`.

**They are not all going to be kept.** Most of them are an exercise: a language that is easy to
compile to one ecosystem and impossible in another has usually made a type decision it did not
know it was making, and the cheapest way to find such a decision is to try. Backends are how the
type model gets audited, not a compatibility promise, and one may be dropped once it has stopped
saying anything new.

That framing is what makes a target worth adding or not. A seventh imperative dynamic language
would add cost and no information; what earns a slot is a target that is unlike the ones already
there in some way the type model has to survive.

Go is the first target that is statically typed *and* has no runtime type information, which
changes what an emitter has to do rather than just how it spells things. See
[a statically typed target asks for the types the checker already has](research-log/a-statically-typed-target-asks-for-the-types-the-checker-already-has.md).

Python is the first with exact unbounded integers *and* floored division, so it lands in the
emulated bucket for two separate arithmetic decisions at once and shows they do not compound: the
32-bit rule is one modulo, and truncated division is a sign fixup over `//`. jq needed a split
into 16-bit halves for the first only because a double loses the low bits of a 62-bit record,
and Lua needed a fixup for the second only. Python needs both and neither is expensive.

It is also the target where the type model costs least to reach. A record is a dict, which is
what `json.loads` already returns, so reading input is the parse and nothing else, where Go
needs two declared structs and a decoder for the same value.

It found no defect. See
[a backend that finds nothing is evidence only if it is different](research-log/a-backend-that-finds-nothing-is-evidence-only-if-it-is-different.md)
for what that is and is not worth.

### The conditional is an expression, spelled Python's way

`then if condition else otherwise`, sitting between `|` and comparison, right-associative:

```
[1, 2, 3] | map(
    "FizzBuzz" if . % 15 == 0 else
    "Fizz"     if . % 3 == 0  else
    "Buzz"     if . % 5 == 0  else
    str(.)
)
```

jq's `if ... then ... else ... end` was the alternative and was rejected. The objections to
Python's form both dissolved on inspection. Precedence is one line: the conditional binds tighter
than `|` and looser than comparison, so `a if c else b | f` groups as `(a if c else b) | f` and
`x | a if c else b` groups as `x | (a if c else b)`. Python puts its ternary below `|` only
because there `|` is bitwise or. And chaining reads well laid out as above, with the values in
one column and the conditions in another, which for a transformation language means a reader can
see what a branchpoint produces without reading a single condition.

The deciding point is that this language has no statements. jq's form is an expression too, but
shaped like a statement, with `end` closing a block that nothing else in the language has.

### Output needs no side effect

A top-level `Str` already prints raw, so a string containing newlines is line-oriented output.
What was missing was not a way to print but a way to build that string, which is `unlines`, a
pure conversion from `Vec<Str>`. FizzBuzz to a hundred is then an ordinary program:

```
unlines(
    range(100) | map(. + 1) | map(
        "FizzBuzz" if . % 15 == 0 else
        "Fizz"     if . % 3 == 0  else
        "Buzz"     if . % 5 == 0  else
        str(.)
    )
)
```

Adding a `print` instead would have been the language's first side effect, and a much larger step
than it looks. There is no answer to what `map(print(...))` returns in an expression language;
ordering would become observable, which is exactly what stops a map being reordered or
vectorised; and the effect would not be in the type, which is what principle 2 exists to prevent.

`unlines` is named for Haskell's rather than being called `lines`, because `lines` is spoken for
by the splitting direction that `stdin.lines` needs. The general `join(over, with)` cannot be
written at all: functions are unary, so a second argument means passing a record, and there is no
record literal to build one with. That is
[a type you can declare but cannot build](research-log/a-type-you-can-declare-but-cannot-build.md)
biting somewhere a user would actually reach.

`range(n)` is zero-based, matching jq, Python, and this language's own indices, which is why the
example shifts with `map(. + 1)` rather than starting at one.

**The condition is exactly one `Bool`**, which is the claim
[the safety section](#why-cardinality-in-the-type-is-the-safety-mechanism) makes. `"a" if 1 else
"b"` does not typecheck, where jq would run both branches and hand back two answers. Both arms
must agree, since the whole thing is one expression with one type.

## What the prototype showed

A working compiler exists: `plans/` has the build order, `research-log/` has the findings, and
the language it accepts is the one described above minus the effect layer, object construction,
and everything listed under prototype 1's exclusions. It runs on three backends -- Lua,
JavaScript, and native through LLVM -- and a corpus of 22 programs is checked to produce
identical output on all three, with disagreement between backends counted as its own kind of
failure.

That produced evidence for questions that had been argued rather than tested. Recorded here as
what happened, not as verdicts; the statuses in [the open questions table](#open-questions) are
still yours to move.

### Stream lowering does not block a backend that does not stream

[The stream-lowering question](#q5-stream-lowering-strategy-across-the-three-backends) was recorded as blocking all backend work, and its
detail said the strategy must be decided before any backend is written. Three backends are
written and it was never touched.

The reason is that prototype 1 has no effect layer, so every program has statically known extent
and lowers to a counted loop on any target, including one with neither coroutines nor
generators. It blocks *streaming* backend work, which is a much narrower claim, and it means the
window in which backends are cheap is exactly the window before streams exist. That row is
corrected rather than proposed, since it is a fact about what the repository now does.

### The one-way layer shift held, and it has a price

Prototype 1 implemented no value-to-effect shifter at all, taking
[the one-way shift proposal](#q13-does-the-layer-shift-run-only-one-way-with-no-value-to-effect-operator) at its word to see what would break. Nothing needed one,
and every program still typechecked.

What it cost is that three of jq's defining operators came out trivial. `.[]` is the identity on
a `Vec`, so the same program compiles to byte-identical code with and without it, which the test
suite asserts. `,` has no meaning as an operator, because at the value layer it would build a
`Vec` and `[...]` already does. And `|` is ordinary composition rather than a map. They get
meaning back only where extent is genuinely unknown.

The question this raises is not whether the proposal is coherent, because it is. It is whether a
language in which `.[]` does nothing is still recognisably in the jq family. Written up in
[a pure value layer dissolves jq's iteration operators](research-log/a-pure-value-layer-dissolves-jqs-iteration-operators.md).

### Vectorizability fell out of the layout without being declared

[Whether vectorizability is visible in the type](#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization) gains an argument for staying silent.
Under struct-of-arrays, `select` binds `.` to a position rather than a value, so `.age >= 18`
compiles to `ages[i]` and nothing materialises an element. The vectorizable form is what falls
out of compiling the obvious thing against that layout: no pass recovered it, and nothing in the
type had to declare it.

Not decisive, but it is a data point against paying for a second effect to report something the
layout already provides.

### Masking now has an implementation to argue with

[What select returns](#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy) and
[whether dense and masked vectors are distinguishable](#q22-are-dense-and-masked-vectors-distinguishable-in-the-type) were open in the abstract.
`select` is a copy today: it builds a mask and then compacts every column with the same surviving
indices. Under struct-of-arrays a masked view is visibly the cheaper option, because compaction
is the only part that touches element data at all. Still open, but open against something
measurable. See
[SoA is cheap until something wants a whole element](research-log/soa-is-cheap-until-something-wants-a-whole-element.md).

### The native backend is built

[The backend choice](#q15-backend-llvm-via-inkwell-cranelift-or-both) is demonstrated rather than leaning. LLVM through inkwell,
against LLVM 22.1. Native output is an object file plus a linked C runtime, since LLVM does not
link, and string concatenation, integer formatting and JSON parsing all want C rather than
hand-written IR.

### Three string representations now disagree in a specific place

[The string representation question](#q16-string-representation-given-wtf-16-on-the-js-target) is concrete. Lua holds bytes, JavaScript holds
UTF-16, and the native backend holds a pointer and a length over bytes. They agree on ASCII and
are not guaranteed to agree beyond it. `<` on `Str` is where that surfaces first, and it
typechecks today.

### Two claims above are contradicted by what got built

The annotation rule is stated as a rule about lambdas. It is a rule about a *class* of
expression: `input` has no type of its own and can only be checked against an expected one, and
an empty `[]` has the same shape. Three instances, one rule, and every future form of the kind
gets it without a new rule. See
[checked-only forms are a class, not a lambda rule](research-log/checked-only-forms-are-a-class-not-a-lambda-rule.md).

Record types could be declared and not built. A brace occurred in type position only, so the sole
record a program could hold arrived from `input`, which made records and input one feature rather
than two. See
[a type you can declare but cannot build](research-log/a-type-you-can-declare-but-cannot-build.md).
That is what the next section settles.

`input` is not `stdin`, and is scaffolding rather than a decision.
[The batching section](#the-admissible-input-set-and-where-batching-comes-from) gives `stdin` the
type `Stream<Vec<T>>` with its batching visible in the type, and the worked examples throughout
say `stdin.lines`. What got built is `input`: one value, read whole, validated in Rust before any
backend starts, and with no type of its own at all. That is not `stdin` with features missing. It
is a different construct standing where `stdin` will go, and it was the right trade -- the
absence of an effect layer is exactly what made six backends cheap, and the 1.5 plan says so.

The cost is that it invites being built on. Anything that names, types, or generates a codec for
`input` is designing against scaffolding, and dies when stdin becomes a stream: annotating a
stream with the type of one of its values is not a thing. A type alias is safe because it says
nothing about how a value arrives; `input: Db` was not.

## DECIDED: records can be built, and a record is how several arguments travel

Settled by grilling against the glossary rather than by measurement. Nothing here needed a
benchmark: every candidate answer was already implied by something the language had committed to,
and the work was finding which commitment applied.

### The form

`{name: .n, age: .a}` is a **record literal**, the inverse of a projection.
[CONTEXT.md](CONTEXT.md) carries the term and its counterpart.

It synthesises structurally. `{a: 1, b: "x"}` is `{a: Int, b: Str}`, two records with the same
fields are one type as they already were, and nothing is declared or named. Whether named
types should exist, and whether a name would create a distinct type or only an abbreviation, is
untouched: a nominal type would need its literal ascribed anyway, so it could never have used a
bare brace, and deciding it later costs nothing.

### Why a record and not a map

The glossary already separates the two by where the keys are known, and five built things need
them known to the compiler:

- the type grammar gives each field its own type, which one value type cannot express
- a `Vec` of records is one column per field, which is the invariant that produced the
  pointer bug fixed in the native backend
- the Go backend declares a struct per record type and has nothing to declare for a map
- the printer enumerates fields from the type, in [declared
  order](#decided-record-fields-keep-their-declared-order), which is what stops six backends
  disagreeing about key order
- `.name` is checked, so a missing field is a compile error rather than a failed lookup

A map is a different type with different operations, whose lookup yields `Opt`. Worth having for
grouped results and genuinely dynamic keys, and not this.

### One meaning, and `map` is the only thing that crosses a dimension

A spec is what an *access* says about a dimension, and a literal is not an access, so it has no
dimension to spec. `map({...})` is how a record meets a dimension, and there is no `db[]{...}`.

Projection already has two spellings, `db[].n` and `db | map(.n)`, so symmetry was a real
argument for giving assembly two as well. It loses to the cost: a brace that means one thing
alone and another thing after a spec is the ambiguity this design keeps refusing, and `map` is
already primitive precisely because there is no effect layer to derive it from.

### `{}` is legal where `[]` is not

A record literal answers what it is from its contents alone, so it never needs its position to
say. That is true even of the empty one: a record's type is the names and types of its
fields, and having none is a complete answer. `{}` is `{}`.

The `Vec` literal cannot do this, because an entry is where an element type comes from and an
empty one has none. So `[]` remains a form whose type must come from its position, and
`{items: []}` fails for that reason rather than for anything to do with records.

**That gap is real and pre-existing.** The class of position-typed forms is described in
[checked-only forms are a class, not a lambda rule](research-log/checked-only-forms-are-a-class-not-a-lambda-rule.md),
and the checker implements exactly one member of it: `expect` special-cases `input` and falls
through to synthesis for everything else, so `[]` fails in every position including the ones with
an expected type in plain sight. Function bodies compound it, being synthesised and then compared
to the return annotation rather than checked against it. Record literals do not make this worse
and are not the place to fix it.

### Punning is out

`{name}` for `{name: .name}` is jq's most-used shorthand and is not being adopted, for a reason
better than conservatism: it would answer a question by abbreviation. Narrowing a record to some
of its fields is arguably its own operation, the way `select` narrows a dimension, and the
glossary has no term for it because the language has not decided. Sugar that quietly implements
one answer makes the question harder to ask.

The worked example does not need it either. `{message: .commit.message, name: .commit.committer.name}`
has names that differ from the paths they come from, which is the ordinary case.

### Functions stay unary, and a record is how several arguments travel

**This is the decision most likely to look arbitrary later, so it gets the most detail.**

`Sig` is one parameter and one result, and every backend emits unary functions. A second argument
therefore means a record:

```
fn join(a: {over: Vec<Str>, with: Str}) -> Str
```

The alternative was real parameter lists, which cost a change to `Sig`, to `Func`, to the call
form, and to all six emitters, and would then leave two ways to pass two things.

What decided it is the call site rather than the cost. `join(", ")` in jq says nothing about
which argument is which, and every two-argument builtin in every such language re-poses that
question. A record answers it once and structurally, because fields are named and order does
not matter. Named arguments are not a feature here; they are what passing a record looks like.

### Argument parens are optional when the argument is a record literal

```
join {over: names, with: ", "}

db | map {
    message: .commit.message,
    name:    .commit.committer.name
}
```

Unambiguous, because `{` cannot start an expression and cannot follow one, so `ident {` is a
syntax error today and giving it a meaning takes nothing away.

The rule is about the argument and not about calls, which matters: `map` and `select` are keyword
forms with their own parens rather than calls, so a rule phrased about calls would have missed
the case that motivated it. Parens stay for everything else, so `map(.n)` and `str(x)` are
unchanged.

This is sugar, and it was accepted where punning was refused, which is worth being explicit
about. Punning hides an unanswered question. This hides nothing: it makes the record the
spelling of named arguments, which is what the previous section decided it already was. Two
spellings for one call is the price, and the unary-function decision is worth less without it.

### What it costs the native backend

`tl_map_new` allocates one column, so a `map` whose body returns a record would violate the
struct-of-arrays invariant at a second site and reproduce the pointer bug the field access just
had. `map` has to allocate one column per field and write column-wise, and the first test of
it should be `map {a: {b: .x}}`, which is the shape that broke.

## DECIDED: a minimal cut of streaming input, pull-based, one new keyword

Not an attempt to settle [Q1](#q1-streams-first-class-values-or-evaluation-level-multiplicity)
in general. Q1 leans evaluation-level -- no `Stream<T>` type constructor, multiplicity born from
I/O and dying into values -- and that leaning is close to right but general enough to be
expensive to get wrong. What is decided here is the smallest concrete instance that neither
commits to nor contradicts it: one monomorphic, unparameterised primitive, not `Stream<T>` over
an arbitrary `T`.

### Pull, not push

Every mature abstraction whose job is specifically "do not let the producer outrun the consumer"
turns out to be pull: Rust's `Iterator`, Python's generators, JavaScript's async iterators (the
layer people actually write against, even though the *raw* primitive underneath, `Readable` in
flowing mode, is push and is Node's own documented backpressure footgun), and jq itself --
verified empirically, `limit(3; range(100000000000))` returns three values instantly rather than
attempting to materialise an impossible sequence. Push-based designs need a second, explicit
protocol bolted on to get the same property (Node's `pause`/`resume`, Reactive Streams' `request`
demand signal); pull gets it as a consequence of the calling convention, for free.

Two costs of this choice, named rather than discovered later. Fan-out -- using one stream in two
places -- is not supported; a pulled item is consumed once, and buffering it into a `Vec` first
(`collect`) is the only way to reuse it. Concurrency and overlap between pipeline stages are not
supported either; a pull chain does not run ahead of the consumer, so nothing overlaps with
anything. Both are named non-goals for this cut rather than gaps found later. What is not lost:
cross-process overlap, the kind `grep foo | wc -l` already gets from the kernel, is a property of
not pre-reading stdin into Rust before a subprocess backend runs, which this cut does -- an
architectural fact rather than a language feature.

### `lines`, `collect`, and nothing else exposed

```
unlines(collect(lines))
```

is the acceptance program: `cat`, echoing stdin back unchanged. `lines` is a bare keyword, zero
arguments, of one new monomorphic type with no spelling in the type grammar -- unlike `Str` or
`Int`, it cannot be a function's declared parameter or return type, which is what makes several
downstream checks unconditionally true rather than needing their own case: a `Lines` value can
never cross a function boundary, so a function body's synthesised type can never disagree with a
declared return type by containing one.

No general `stdin` value is exposed. `input` (JSON, one document, read whole, validated before
any backend starts) is untouched, and a program may not use both: forced by jq specifically,
since raw-input mode (`-R`), needed for `[inputs]` to read lines rather than JSON, changes what
the whole invocation means, not just one call, and cannot coexist with `.`-is-the-document mode
in one run -- verified against real jq before the rule was written.

`lines` may be referenced once, checked by the same mechanism that already caught `input` being
used at two disagreeing types, simplified to "used at all, twice." Python's generators are the
cautionary example on the other side of this decision: consuming one twice silently returns
nothing, no error, and Rust's ownership model catching the equivalent mistake at compile time
(usually) is closer to what was chosen here on purpose.

`Lines` is refused from a Vec, a record field, or the program's own printed result, checked at
three separate points rather than one, because a single "does the top-level result contain
Lines" scan does not catch every path: `collect([lines][0]!)` never makes Lines the program's own
result, but would have reached `to_slot`/`read_slot`/`go_type`'s struct-of-arrays or
closure-typing machinery with a type those never expected, on every backend, had the containment
check not been pushed back to where a Vec or record literal is built.

### The line rule, checked rather than assumed

Split on `\n` only. Strip it from each yielded string. No trailing empty entry when input ends in
`\n`. The final line is still yielded even with none, which is deliberately not `wc -l`'s
mistake: `wc -l` counts newline characters rather than lines, so a file missing its last one is
undercounted -- verified directly, `printf 'a\nb'` reports 1 rather than 2. A bare `\r` is left
alone as ordinary content, matching `jq -R` and Python's own raw stdin iteration, neither of
which treats CRLF specially; Go's default `bufio.Scanner` split function does strip it, verified
directly, so the native Go split function is `bufio.ScanLines` with that one line removed rather
than the stdlib default.

Every other backend's own native mechanism already agrees with the rule once verified: `getline`
(native), `io.lines()` (Lua, confirmed reading the real process stdin directly under `mlua`'s
default, unrestricted `Lua::new()`), raw `for line in sys.stdin` (Python), and a hand-written
`fs.readSync` chunk loop (JavaScript, which has no synchronous line reader built in -- the
well-known reason competitive-programming Node code writes this exact idiom by hand).

### What "each backend reads for itself" actually meant to build

Two thirds of the six backends are separate OS processes Rust spawns (native, jq, Go, Python,
JavaScript); the toylang compiler is never a runtime dependency of what any of them emits, only
of producing it. The sixth, Lua, is not a process at all: `mlua` embeds the interpreter in the
same address space as the Rust harness, so "reading stdin" there is a function call, not IPC.

For the five subprocesses, genuine streaming meant stopping Rust from reading all of stdin into a
`String` and re-`write_all`-ing it once, and instead connecting the real stdin straight through
(`Stdio::inherit()`) so the OS does the buffering it was always going to do. `main.rs`'s existing
gate -- read stdin only if `input` is used -- already did the right thing for `lines` by
accident, since neither construct needs Rust to touch the real stdin at all.

What is fed depends on whether a caller supplied fixture text. `Some(text)` (a corpus case, most
often) is piped in verbatim and each backend does its own real splitting against it, which is the
only way the corpus can prove all six backends' splitting genuinely agrees rather than testing a
Rust-side reimplementation instead. `None` (the real command-line case) connects the live stdin.
Getting this distinction wrong the first time silently would have meant every corpus case for
this feature passing while testing nothing: the first version skipped writing fixture text to a
`lines` program's pipe entirely, matching `Stdio::inherit()`'s empty-until-corrected mental model
rather than the fixture's actual bytes, and would have reported every case as agreeing on an
empty string.

Lua needed a different mechanism for the same distinction, since `Stdio` does not apply to a
function call in the same process, and `cargo test` runs many tests concurrently on shared
threads, which rules out redirecting the test process's own real stdin even temporarily. The
fixture is written to a file of its own, and the *global* `io.lines` function -- not `io.stdin`
-- is overridden to call the *real* `io.lines`, pointed at that file, so Lua's own line-splitting
still runs, verified rather than replaced by a Rust-side reimplementation feeding a pre-split
list.

### What this does not attempt

No progressive output: a program still computes one final value and prints it once, unchanged
from every other program in the language. Real per-line output, and what a stream of *outputs*
would even mean, is [Q35](#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return)'s
territory and stays there. No concurrency, no event loop, no `async`/`await` -- and not
having them is not merely smaller, it sidesteps two more named historical mistakes: JavaScript's
`Promise` starting eagerly with no cancellation designed in (irrelevant here, since nothing runs
until asked), and the sync/async colouring split that Rust and Python both still carry scars
from (irrelevant here, since there is only one calling convention). A future main loop, if one is
ever built, would not invalidate this: `stdin`/`lines` as a pull-shaped surface over an
event-loop-driven scheduler underneath is exactly how Python's `asyncio` and JavaScript's async
iterators are actually built, precedent rather than a hope.

## DECIDED: one stdin source is the destination

Direction settled 2026-08-28: the three stdin keywords (`input`, `inputs`, `lines`) are
officially transitional. The destination is one raw source with parsing as ordinary,
visible steps -- the design rejected earlier *only* because expected types could not flow
into map bodies, a blocker the type-flow rework removes. Syntax is deliberately not settled
here; it gets its own session once type-flow lands. Until then: no fourth stdin reader, no
new exclusivity rules -- nothing that makes the migration bigger.

## DECIDED: `inputs`, eager, not an answer to Q1 either

jq's own `inputs` -- every remaining JSON value on stdin, one per line -- was the obvious next
question once Field distributing over a Vec turned out to only need what CONTEXT.md's Spec
glossary already calls Keep and Narrow "streamable" for: an iterator, not storage. That
observation does not, on its own, get toylang a lazy `Stream<T>`,
because a program still has to terminate and return one value
([Q35](#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return) again): nothing can
consume a stream incrementally forever today, so the only thing genuine laziness would buy is
memory, not a new capability. `inputs` is eager instead: a bare keyword, typed `Vec<T>`, `T`
inferred from first use exactly the way `input`'s own type is -- read in full, parsed, collected,
before the program body runs. Ergonomics, not the general question; Q1 stays exactly as open as it
was.

A more general alternative was considered first and rejected on a concrete, checkable ground
rather than taste: a `parse(s: Str) -> T` builtin, composing as `collect(lines) | map(parse(.))`,
would have been more reusable (also parsing a JSON string embedded in an ordinary field) and
would have needed no third stdin mode at all. It does not work with the checker as it stands.
`expect()` threads an expected type into exactly one form, `Expr::Input`; a `map` body is always
`synth`ed bottom-up, with nothing flowing in from whatever consumes the map's result. `parse(.)`
inside `map` has no expected type to resolve `T` against, the same hole an empty `[]` literal
falls into and for the same reason. Fixing that generally -- expected types flowing through `Map`,
at minimum -- is real, separate checker work, not scoped to this feature, and this would only be
its first forcing use case.

All three stdin readers are now mutually exclusive, not just `input` and `lines` as before, and
the third leg was not a judgment call: Python's `input` reads all of stdin to EOF as one buffer
(`sys.stdin.buffer.read()`) before parsing, leaving nothing for `inputs` to read afterward even in
principle, and jq needs a different invocation flag for raw-line mode (`-R -n`) than for
parsed-JSON mode (`-n` alone) -- one process cannot run with both. `inputs` and `lines` could not
have coexisted on either backend regardless of what this document decided.

Native's `tl_read_inputs` (`runtime/toylang.c`) reuses both existing pieces rather than needing a
new parser: the same `getline` loop `tl_collect_lines` already has for "one line at a time," and
the same descriptor-driven `tl_parse` `tl_read_input` already has for "one JSON value, this
shape" -- `tl_parse` was already structured around an arbitrary buffer range, not "all of stdin,"
so looping it per line needed no changes to it at all. The one new piece of care: `tl_parse` hands
a record back as one packed blob, not columnar data, so `tl_read_inputs` spreads it into the
struct-of-arrays layout itself -- a fourth site for the invariant
[three other construction sites](research-log/one-invariant-three-independent-construction-sites.md)
each violated independently, verified here against a corpus case
(`tests/corpus/inputs_records.yaml`) rather than found the way those three were.

Superseded on one point: the premise above, that nothing can consume a stream incrementally so
laziness would only buy memory, stopped being true when the fused `jsonlines(f(inputs))` loop
shipped. `inputs` is no longer typed `Vec<T>`; it is a source of `Stream<T>`, and eager
consumption is spelled `collect(inputs)`. See
[the streams decision](#decided-stream-is-the-effect-layer-typed). The rejection of `parse`
and the three-way stdin exclusivity above are untouched by this.

## DECIDED: `jsonlines`, and the jq tutorial reproduces in full

jq's own tutorial (jqlang.org/tutorial) has six filters. Five already matched real jq byte for
byte: `.`, `.[0]`, `.[0] | {...}`, `[.[] | {...}]`, and the full nested version with `parents`.
The sixth, `.[] | {message: ..., name: ...}`, printed as impossible for a real reason: jq's `.[]`
is lazy, so a later element's failure does not erase what already printed for earlier ones --
verified directly, `[1,2,0,3] | .[] | 10/.` in real jq prints `10` then `5` before erroring on the
zero. toylang's `Vec` is eager and whole, exactly as the streaming section above kept it: nothing
prints until everything exists.

That was the truth about step 4's *failure* semantics, and it is still true. It was not, on
inspection, the truth about its *success* semantics. `jq -c` printing one JSON value per line and
`jq -c` printing one array are the same bytes with the brackets and commas swapped for
newlines -- verified, `.[] | f` and `[.[] | f][]` produce identical output on the happy path. So
what was missing was not an effect layer. It was a printer.

`jsonlines(v: Vec<T>) -> Str`, for any printable `T`, joins each element's compact-JSON encoding
with `\n` instead of wrapping the whole `Vec` in `[...]`. It is the first polymorphic builtin --
every earlier one (`str`, `range`, `unlines`, `collect`) has one fixed signature -- so it is
checked directly in `synth`'s own `Call` arm rather than through the fixed-signature table, the
same way `map` and `select` are. Every backend already had the per-type encoding this needs, in
the same function that prints the program's own result; `jsonlines` is that function invoked
mid-program instead of only at the top, joined by `\n` in place of `[`, `,`, `]`.

```
jsonlines(commits(input) | map {message: .commit.message, name: .commit.committer.name})
```

reproduces jq's step 4 exactly, on all six backends, verified against real jq.

Finding the right spelling exposed a real bug rather than causing one: `jsonlines` was the first
thing to print a `Vec` of records with more than one element built directly as a literal, and
native's `vec_lit` wrote every record's whole pointer into column 0 instead of spreading its
fields across columns -- the same struct-of-arrays invariant that field access and `map` had each
already been fixed for, at a third, independent site. See
[one invariant, three independent construction sites](research-log/one-invariant-three-independent-construction-sites.md).

## DECIDED: `f x` reads as `f(x)`, but only where an expression begins fresh

REVISED (2026-08-28): bare application is no longer a confined third spelling -- it is **the
default calling style**, with `f(x)` as the explicit disambiguator. Three changes carry that:
the root-position confinement and the definition-body suspension are replaced by a same-line
rule (a bare argument must start on the same line as its function -- the same rule the
record-argument sugar's boundary fix already decided, so `= extent v` works and a next-line
program body is never swallowed); the `ident {` record-argument sugar stops being separate
machinery and becomes ordinary bare application whose argument is a record atom, leaving
exactly two call forms; and the corpus, examples, and docs migrate to bare style where it
reads better, so the language teaches its own default. The `-` exclusion stands (`f -1` stays
subtraction). The section below records the original confinement and its reasoning, which the
same-line rule supersedes.

Every function is unary, so `f(x)` never needed the parens to disambiguate which argument is
which -- only to mark where the argument starts and ends. Parens were doing two jobs: grouping
(`(x + y)`) and marking a call's boundary (`f(x)`), and those turn out to be the same job. `f(x)`
*is* `f` applied to the atom `(x)`, which happens to be a grouped `x`; nothing distinguishes it
from `f (x)`. Once seen that way, the parens around a call's argument are exactly as optional as
the parens around any other atom that does not need grouping: `f x` should mean `f(x)`.

The obstacle was never precedence, once framed correctly. `f x + y` looks ambiguous only if the
grammar has to decide whether `+ y` extends the argument or the whole call. It does not have to
decide, because it does not have to accept the program: `x`, once taken as `f`'s bare argument,
is not an operand of anything, so `+ y` is simply left over and rejected as trailing garbage
rather than resolved either way. This is not a rule bolted on top -- it falls out of a bare call
never being reachable from `operand`'s own recursive tree (`unary`, `postfix`, `atom`), only from
`expr`'s outermost dispatch. `f(x) + y` stays legal, because the parenthesized form is an
ordinary atom, reachable from anywhere `atom` is; only the bare, parenless spelling is confined
to root position. Chaining follows the same recursion: `f g x` is `f(g(x))`, right-associative,
because `f`'s bare argument is itself allowed to be another bare call -- and, since toylang has no
first-class functions or currying, that is the only reading that could ever typecheck anyway, so
nothing was given up by not entertaining the other one.

`-` is the one place a token is both a legitimate binary operator and could plausibly start an
argument (negation). It is excluded from starting a bare argument entirely, so `f -x` stays
`f - x` -- the same resolution Haskell gives the identical clash -- rather than adding a rule to
prefer negation. If `f` is a function name rather than a real variable, the checker rejects it as
an unbound name, a plain error rather than a silently wrong parse.

Enums added a second exclusion, on the callee side this time: only a lowercase name can be a
bare call's function. Functions are values under the casing rule, so a capitalised callee was
already impossible to satisfy; stating it in the parser is what keeps `Shape.circle`, the
[qualified variant spelling](#decided-enums-nominal-and-json-native), from being swallowed as
`Shape (.circle)`, since `.` also starts a bare argument. Nothing legal was lost -- a
capitalised bare call could never have typechecked -- but the rule is a parser fact now rather
than a checker consequence, which is why it is recorded here.

`select` and `map` are not special syntax any more. They used to be keyword tokens with their own
grammar production; now they are ordinary identifiers, reserved by name the same way `jsonlines`
already was, checked inside `Call`'s own `synth` arm rather than through dedicated AST nodes. The
parser no longer knows anything about them.

Building this surfaced a real grammar hole rather than causing one: the file grammar is
`(fn | type)* body`, and a definition's own body is the one place in the whole grammar where an
expression is parsed with no delimiter marking where it ends -- not preceded by `|`/`(`/`[`/`{`,
not followed by a required closing token the way the program's own `body` is bounded by `Eof`.
`fn f(x: Int) -> Int = x` followed by `f(1)` stopped compiling: `x`, followed immediately by `f`
with nothing between them, was read as `x` applied to `f(1)`. The fix is a parser flag, off for
exactly a definition's own undelimited top-level chain and switched back on the instant a real
delimiter (`(`, `[`, `{`) is entered, since a closing token bounds those regardless of what is
outside them. See
[juxtaposition is unsafe at any undelimited boundary](research-log/juxtaposition-is-unsafe-at-any-undelimited-boundary.md).

## DECIDED: a rudimentary module system, one prelude file and `pub`

`unlines` used to be a `tir::Builtin`, needing its own codegen in six backends. It is now ordinary
toylang source in `prelude.toy`, marked `pub`, and every `pub` definition there is always
available to a program -- there is no import statement, and a program cannot yet name what it
wants from the prelude or export anything of its own for another file to use. `pub fn` is parsed
(and stored on `ast::Def`) everywhere a `fn` is, including in an ordinary program, but it has no
effect there yet: nothing imports from a program file today.

Non-`pub` is not yet a working privacy boundary. A `pub` prelude function can only be fully
self-contained, calling only compiler builtins and itself -- it cannot call a private prelude
helper, because a non-`pub` definition is never merged into any compiled program at all, not even
one that merges in a `pub` sibling from the same file. Real scoping (a `pub` function using a
private one, without exposing it to callers) needs the checker to track which file a definition
came from and enforce visibility per call site, which does not exist yet. This cost nothing today,
with one function in the prelude and no helpers of its own, but it is the reason a second prelude
function that needs a private helper cannot be added yet without that machinery first.

Merging every `pub` definition unconditionally reopened a problem `unlines` had already solved
once, by scanning the program's source text for the name before merging it in. That approach does
not extend to "always merge everything": an unused prelude function would sit in every compiled
program's `Program.funcs`, which is exactly what `tags::node_types` walks and what every backend
turns into output. The fix generalizes past the textual approximation: `check::check` now prunes
`Program.funcs` to whatever the program's body can actually reach, directly or through a call a
reached function itself makes -- the same treatment an unused function the program wrote itself
now also gets, which nothing pruned before. See
[named functions kept an open question open](research-log/named-functions-kept-an-open-question-open.md)
for the related choice, made the same way, to add a capability as a name rather than by extending
existing syntax.

`prelude.toy` is parsed as a module -- `parse::parse_module`, a second entry point next to
`parse::parse` -- rather than as a program with a throwaway trailing expression, since it is a
real, checked-in file meant to be read: a module is zero or more `[pub] fn` definitions and
nothing else, with no body to fake.

## Mutation as an optimization: privileged and shared references

TODO (user, 2026-08-28), queued for its own grilling. `Vec` is immutable and should likely
stay that way *semantically* -- but a compiler-internal notion of **privileged references**
(exactly one reference provably exists) versus **shared references** could make mutation an
optimization without importing Rust's borrow checker or any runtime reference counting:

- When `x = [0, 5]` has provably one user and flows into something like `x + [-1]`, the
  backend may mutate in place. When other users exist, a copy is made -- and only at the
  moment an actual mutation happens, not eagerly at the branch. Branching that shadows a
  variable can therefore create copies lazily.
- The analysis should fall out of the existing syntax statically: no runtime machinery.
- Function-internal shadowing becomes the idiom for "mutation" that can never contaminate
  the caller.
- Calls are where it gets interesting: a function may receive a privileged or a shared
  reference, and each *branch* of its body may return an inherited reference or one created
  internally -- so one source-level function breaks down, per call site, into a
  specialization with specific promises ("composite function call"), tracked internally by
  the type system and never surfaced in syntax. Unary functions keep this tractable: per
  overload, one privileged/shared bit in, and per-branch provenance out -- no combinatorial
  parameter matrix.
- The payoff is exactly [Q10](#q10-is-uniqueness-analysis-in-scope-for-deciding-when-a-lens-materializes)'s
  question answered from the other side: native's vector implementation (and other targets)
  gets honest in-place mutation, and the when-does-a-lens-materialize question gains its
  mechanism. Related: [Q14](#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy)
  (select's copy question) and the heap/stack model the draft has so far avoided inventing.

Prior art to weigh at the grilling: Clean's uniqueness types (static, but surfaced in
signatures -- this sketch deliberately hides them), Koka/Lean's Perceus and Roc's
opportunistic in-place reuse (both runtime refcount-based -- this sketch deliberately
refuses that), and functional-but-in-place compilation generally.

## DECIDED: record fields keep their declared order

Records print in the order their type declares, not sorted: `{name: .n, age: .a}` prints
`{"name":...,"age":...}`. Order lives in the *type* -- record fields are static, so every
value of a type prints identically and determinism survives -- and input is normalized to
declaration order on read; arrival order is not data. (For a future arbitrary-keyed Map type,
per-value order is a separate question, deliberately not prejudged here.) This replaces the
alphabetical order the printers shipped with, which existed to keep seven backends agreeing
cheaply: the same agreement now rides on declaration order, which is what jq users and
downstream diffing actually expect. Migration: the printers and the native/Go layouts (whose
column order was sorted position) move to declaration position, and the record-printing
corpus expectations re-pin.

## DECIDED: record field order is not type identity

kantord/toylang#60, ratified in the #24 wizard round. `{a: Int, b: Int}` and `{b: Int, a: Int}`
are one type: the checker compares a record's fields as a set. This amends the determinism claim
above -- "every value of a type prints identically" now means every value ever checked against
the same declared spelling; two literals of the same type that never meet at a checked position
(a function argument, a return type, a Vec element) can still print in their own, different
declared orders, since nothing forced them to agree. Declared order remains real: it is still
what a value prints in and what the native/Go columnar layouts key on, and it is meant to become
a runtime-queryable accessor (a field_names-style builtin, name and shape not yet decided --
kantord/toylang#63) for serialization and friends, not implemented by this decision.

Implementation: whichever type a value is checked against becomes the order it is rebuilt in
(`check::reorder_record`), so a value crossing a call, a return, a branch, or a Vec literal's
own element always ends up laid out like its declared position expects. The gap this leaves --
a Vec, Opt, or Stream whose element arrives already built with a different order than the
container's declared element type -- needs an actual per-element transform, not a local
rebuild, and is filed separately (kantord/toylang#64).

## DECIDED: Stream is the effect layer, typed

This settles [Q1](#q1-streams-first-class-values-or-evaluation-level-multiplicity), and not by
picking either of its two named options. Streams are evaluation-level multiplicity, as every
argument collected under that question kept concluding -- but the type system now writes that
multiplicity down. `Stream<T>` enters the type grammar as the type *of* effect-layer
multiplicity: it says "this expression yields its entries one at a time as evaluation
proceeds," never "a stream object exists." The two-layer section's confessed tension -- a
cardinality table presenting `Stream<T>` as a type while the layer model treated multiplicity
as evaluation-level -- dissolves, because the two were describing the same thing from two
sides.

What forced the question was not theory but a lie already shipped. The fused
`jsonlines(f(inputs))` loop made programs stream for real, while the checker kept typing
`inputs` as `Vec<T>` and `jsonlines(...)` as `Str`. Whether a program streamed was decided by a
backend-side pattern match (`tir::recognize_fusion`), invisible to the type, and a program one
shape away from the recognized pattern fell back to materializing all of stdin with no
indication anything had changed. That silent cliff, not a missing value type, is the defect
this decision fixes: eager use of stdin now has a visible, greppable spelling, and streaming
use is checked rather than guessed.

### The rules

A `Stream<T>` is born only at a source, flows only through a pipeline, and dies only at an
exit.

**Sources.** `inputs : Stream<T>` (element type inferred from use, as before) and
`lines : Stream<Str>`, replacing the monomorphic `Lines`. Nothing else creates one: there is
no value-to-effect operator, exactly as [Q13](#q13-does-the-layer-shift-run-only-one-way-with-no-value-to-effect-operator)
leans. Decided after the implementation disclosed a cross-function hole (a function reading a
source, called from a mapper body, re-read stdin per element): a source may appear only in
the program's own body, never inside a `fn` definition -- functions receive streams
exclusively through `Stream<T>` parameters. The alternative, tracking source-consumption per
function transitively, was rejected as an invisible effect on every signature, the implicit
shape this design keeps refusing.

**Mappers.** `Stream` is spellable in signatures -- the one thing the `Lines` design
deliberately withheld -- so `fn f(s: Stream<A>) -> Stream<B>` is legal. `map`, `select`, and
projection are cardinality-polymorphic: stream in, stream out.

**Exits.** `collect : Stream<T> -> Vec<T>`, generalized from its `Lines`-only signature, and
`jsonlines`, which becomes a *sink*: legal only as the program's outermost expression, with no
result type at all, since nothing remains that could observe one. Its old `Str` typing was a
placeholder asserting the opposite of what the fused loop does. `collect` is reify spelled as
a word, not a third layer shifter; whether the `[...]` form subsumes it is deferred along with
the vec-literal and [cartesian-vs-zip](#q2-binary-operators-over-two-multi-valued-expressions-cartesian-zip-or-explicit)
questions, because deciding it means deciding how literals interact with effect typing.

**Linearity.** A stream is consumed exactly once. This generalizes the existing "`lines` may
be referenced once" check rather than inventing a discipline, and the strictness runs the
reversible direction: exactly-once can relax to at-most-once later without breaking a single
program, while the reverse tightening would break every program that dropped a stream. The
[ordering question](#q4-can-the-type-express-ordering-over-heterogeneous-streams)'s warning
that linear types "infect the whole system" is about linearity as a discipline over all
values; one second-class type consumed linearly is the `Lines` rule with a type parameter.

**Second-class by construction.** No `Stream` in a record, in a `Vec`, or in another `Stream`.
These are not new restrictions but the existing `Lines` containment rules, which were always
justified the same way: an effect is not a value, so there is nowhere for it to be stored.

### What this deliberately does not decide

[What stdout is](#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return) stays
open. Making `jsonlines` a top-level-only sink removes the placeholder answer without
smuggling in a real one: no `Out` or unit type exists, and stdout-as-value versus
stdout-as-effect gets its own argument another day.

No stream reducers yet. `extent` stays `Vec`-only, keeping its documented no-fold promise; a
fold over a stream is real future work, not a typing tweak. The one eager corpus case,
`total(inputs)`, migrates to `total(collect(inputs))` -- the honest spelling of what it always
did.

### Consequences

`recognize_fusion` stops guessing. Types say what streams; backends compile stream-typed
pipelines as the loops they already know how to emit; an eager fallback for a stream-typed
program stops being a silent behavior and becomes a compiler bug. The corpus cases over
`inputs` change signatures (`Vec` to `Stream` for the two mapper-shaped ones), and rejection
tests are needed for what the type now forbids: a nested `jsonlines`, a twice-consumed stream,
a `Stream` in a record. The output-equality corpus cannot see any of this, the same blindness
that made `tests/streaming.rs` necessary.

## DECIDED: Float is JavaScript's double

`Float`, when it is built, is exactly the IEEE 754 binary64 double that every JavaScript
engine carries: no alternative width, no decimal type. The reasoning is one step long, which
is why this section is short where the `Int` one is not: every backend has this
representation natively, JavaScript and jq have *nothing else*, and the carrying measurements
in [the Int decision](#decided-int-is-32-bits-and-wraps) already established the double's
53-bit integer ceiling as the portable envelope. Choosing anything else means emulating a
second float on the two targets that have only this one, to gain a width nothing asked for.
Recorded as [an ADR](docs/adr/0007-float-is-javascripts-double.md) as well; nothing is
implemented yet, and `3.14` in the values list above is still only notation.

Representation was the easy half. What the decision opens is tracked as
[the float-semantics question](#q37-how-do-floats-print-and-what-are-nan-and-infinity-in-a-json-shaped-value-model)
rather than assumed here.

## DECIDED: match arms compose with `or`, and a guard chain may be honestly partial

FizzBuzz was the case study. The shipped conditional chain and the decided arm chain, side by
side -- the program barely changes shape, which was the point:

```
"FizzBuzz" if . % 15 == 0 else          . % 15 == 0 -> "FizzBuzz" or
"Fizz"     if . % 3 == 0  else          . % 3 == 0  -> "Fizz"     or
"Buzz"     if . % 5 == 0  else          . % 5 == 0  -> "Buzz"     or
str(.)                                  str(.)
```

An arm is a produce-or-decline value: its left side is either a variant pattern
(`circle{r}`) or a Bool guard (`. % 15 == 0`), its right side the produced expression. `or`
composes arms first-match-wins, and it is the *only* spelling of that composition: `//`
retires, and the enum match migrates (`s | circle{r} -> r * r or point -> 0`). One operator,
overloaded by operand kind, exactly as `+` already is over Int and Str. The chain's final
element may be a bare expression, the default. Reserved now rather than discovered later:
the language has no Bool `or` yet, and if one ever lands, arm-`or` binds loosest and a bare
`or` inside an arm body needs parens.

Totality is a hybrid, and the failure mode that shaped it is worth keeping. An enum pattern
chain is closed-world: it must cover every variant or end in a default, keeping the shipped
named-missing-variant error. A guard chain is open-world and may be partial, yielding `Opt`
-- the same answer indexing already gives to "found nothing" -- EXCEPT when an arm's body is
itself `Opt`-typed, which is refused ("add a default"): our `Opt` is untagged, so a partial
chain over an `Opt`-bodied arm would print one `null` for two different absences (arm
declined, versus arm matched and found nothing), the exact conflation
[the field-access question](#q12-on-a-type-mismatch-does-field-access-error-yield-null-or-something-third)
forbids. `map(.valid -> .readings[0])` over `{valid: false, readings: [5]}` and
`{valid: true, readings: []}` printing `[null,null]` is the program this rule exists to
refuse.

REVISED (2026-08-29, kantord/toylang#62): the refusal's premise -- "our `Opt` is untagged"
-- fell with the Opt-as-enum ratification above. A partial chain over `Opt`-bodied arms now
types as `Opt<Opt<T>>`, two honest levels a program can tell apart through `!`; the
two-nulls program is legal and prints `[null,null]`, which is serialization's documented
lossiness rather than a type-level conflation (the corpus case match_partial_opt_arms pins
it). The closed-world half, and guards never counting toward coverage, stand unchanged.

Deferred with their own triggers: matcher values (`int()`, decoders), `and`/`not`,
running a matcher to a `Result`, and deep patterns -- Q27's decode work, when it has
customers. Mixed chains (pattern arms and guard arms over one enum subject) are legal;
guards do not count toward variant coverage.

## DECIDED: enums, nominal and JSON-native

The language gets a Rust-inspired enum: a declared, closed set of named variants, each
optionally carrying a payload. `enum Shape { point, circle{r: Int} }` is the first
user-written type declaration the language has, and it is prioritized ahead of implementing
the stream type-system work, because it feeds nearly everything around it: tagged alternation
for [heterogeneous streams](#q4-can-the-type-express-ordering-over-heterogeneous-streams),
the closed-world half of
[pattern matching](#pattern-matching-is-decoding), and the first real answer to
[the union question](#q25-does-the-language-have-union-types).

### The value is JSON, canonically

An enum value is not an abstract sum with a codec; it *is* a JSON shape, the same way every
other value here is. A payload variant is the single-key wrapper, `{"circle": {"r": 1}}`. A
unit variant is a bare string, `"active"` -- which makes an all-unit enum a string enum, so
`{"status": "active"}`-shaped data, the most common enum shape in the wild, is directly
typeable. Payloads are any single type, with a record when several values travel, the same
rule arguments already follow. Variant names are data (they appear as JSON keys and strings),
so like record fields they are exempt from the capital-means-type casing rule.

The costs were weighed, not missed: tag-field data (`{"kind": "circle", "r": 1}`) is also
common in the wild and is *not* directly typeable as an enum; reaching it needs the codec
layer this decision deliberately does not build. And `Opt` is provably not self-hostable as
an enum -- its canonical form has been value-or-`null` (untagged) since it existed, so Rust's
"Option is just an enum" elegance is unavailable. `Opt` stays built-in.

REVISED (2026-08-29, ratified in the quick-ratifications round, kantord/toylang#62): the
self-hosting proof above stood on the premise that the canonical form IS the value; the
maintainer knowingly dropped that premise. `Opt` becomes a prelude enum over generic enums
(whose first customer it now is, taking the slot reserved for `Result`), tagged in memory,
with `null` surviving only at the serialization boundary -- lossy there by design, the way
serialization is already lossy about every type-level distinction. The two-nulls machinery
relaxes wherever the tag genuinely distinguishes. Plan-first build; the monomorphic
first-cut sentence below is what the plan retires.

### Nominal, closed, exhaustive

The enum's name is an identity, not an alias: exhaustiveness checking has to know which
closed set it is proving coverage of. This decides
[the named-types question](#q34-do-named-types-exist-and-is-a-name-an-alias-or-an-identity)
for enums only; whether *records* get identity stays open.

Consumption routes through the arm syntax
[pattern matching is decoding](#pattern-matching-is-decoding) already sketched, as its first
minimal implementation: variant arms chained with `//`, `.` rebinding to the payload inside
an arm, and the compile-time-total branch that section already reserved for "an enum you
already have an instance of." A match over an enum must cover every variant or end in a
default arm. The `Matcher` algebra (`and`/`or`/`not`, dynamic `Json` decoding) is not part of
this slice; [Q27](#q27-does-pattern-matching-need-a-separate-matcher-type-distinct-from-result)
is untouched.

### Construction and naming

REVISED AGAIN (2026-08-29, the auto-matchers grilling, kantord/toylang#47): the Aug-28
revision below stands on casing and on variant-names-as-types, but its expression-position
reading flips. **A capital name in expression position is the type's derived matcher, not a
constructor.** The type is `Active`; its matcher function is also `Active` (the two roles
never collide, one lives in type position, one in expression position); its raw constructor
is lowercase -- for a unit variant that constructor is `active`, a plain constant, no call
(nothing is consumed, so nothing signals application), and for a payload variant it is the
unary function `circle{r: 1}` exactly as before, lowercased. Only derived matchers may start
with a capital letter; they are not human-definable. So the Aug-28 example's body spelling
`= Active` becomes `= active`. The matcher's domain is variants now, designed so untagged
unions can join later (the #29 thread); matchers are first-class and tagged -- Q27's
`Matcher` type is SETTLED by this grilling, not just leaning -- so a non-match is a value
`or` composes, never a bare `null`. Still open, queued as the next decide row: measuring or
enforcing full-coverage matches through first-class matchers, one arm serving several
patterns (`alt`-composition), and whether the same algebra genuinely builds parser
combinators (Q30's claim, now with a concrete surface to test against).

REVISED (2026-08-28, superseding the lowercase rule below): **variants are capitalized, and
a variant name is a type** -- each variant is a subtype of its enum, so a signature can
promise one specific variant:

```
enum Status { Active, Paused }

fn initial() -> Active = Active
fn describe(s: Status) -> Str = s | Active -> "on" or Paused -> "off"
```

The JSON form is the name verbatim (`"Active"`), so wild lowercase data
(`{"status": "active"}`) is no longer directly typeable and waits for the codec layer -- a
knowing reversal of the original bare-string argument, traded for signatures that can narrow.
This also restores the casing rule's purity: capital means type, with no data-exemption
needed, because variants now are types. Subtyping (a variant accepted where its enum is
wanted) enters the type system here and nowhere else yet; its exact rules are the build
task's plan to draft and this section's next revision to record. The original decision below
stands for everything else (wrapper payloads, constructors as application, exhaustiveness).

Declaring an enum derives one constructor per variant: `Circle{r: 1}` is ordinary
application, `Active` an ordinary named constant. Payloads accept any single type, so a
record payload has two spellings -- `Circle{r: 1}` and `Circle({r: 1})` -- because the
constructor is an ordinary unary function and the braces form is the same record-argument
sugar every call has. The braces spelling is the one the checker hints. A bare variant name resolves while exactly
one enum in scope declares it; a collision is a loud compile error naming the candidates, and
the qualified spelling is `Shape.circle` -- type-dot-variant, which the casing rule already
makes unambiguous, so no new syntax exists. Typing a literal directly as an enum
(`"active" : Status` with no constructor) is deferred: it needs expected types flowing down
through synthesis, and it is recorded here as the second forcing case for that rework, after
the one [the `inputs` decision](#decided-inputs-eager-not-an-answer-to-q1-either) documented
for `parse`.

First cut is monomorphic: no type parameters. The named motivations all work without them;
generics' first real customer is `Result<T, E>`, which belongs to the decoding work.
RETIRED (2026-08-29, kantord/toylang#62): generic enums shipped with `Opt<T>` as the first
customer, per the ratification above -- declarations take parameters, `Name<...>` resolves by
substitution, and constructors infer the instantiation from their payload or take it from the
position's expectation. `Result<T, E>` inherits the machinery instead of motivating it.

## Open questions

Tracked here rather than scattered through the document. Status is one of OPEN (no preferred
answer), LEANING (a preferred answer exists but is not committed), BLOCKED (waits on another
question), or SETTLED (answered, and the answer is written into the document above). Add new
ones at the bottom and keep the numbers stable, since other sections cite them.

Settled questions stay in the table. A tracker that only lists what is unresolved cannot be
checked for completeness, and the settled entries are what stop a decision being relitigated.

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
| [Q20](#q20-how-are-blocking-operators-sort-groupby-joins-classified) | How are blocking operators (`sort`, `group_by`, joins) classified? | SETTLED, a trait with no lawful stream instance |
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
| [Q37](#q37-how-do-floats-print-and-what-are-nan-and-infinity-in-a-json-shaped-value-model) | How do floats print, and what are NaN and Infinity in a JSON-shaped value model? | OPEN, blocks implementing `Float`; the representation itself is decided |

[Multidimensional vectors](#q9-are-vectors-multidimensional-with--as-projection) is the one
question still capable of changing the two-layer section, now that
[streams](#q1-streams-first-class-values-or-evaluation-level-multiplicity) are settled, so it
should be resolved before that section is treated as stable.

## Question detail

### Q1. Streams: first-class values, or evaluation-level multiplicity?

SETTLED: evaluation-level, and typed. `Stream<T>` is the type of effect-layer multiplicity,
second-class and consumed exactly once, not a value type; see
[the decision](#decided-stream-is-the-effect-layer-typed). The earlier reasoning stands:
evaluation-level keeps
values finite and acyclic, and lets streams compile to loops rather than heap-allocated
iterators. The base-functor formulation's *remainder*, which is a value, is exactly the
first-class-stream cliff the second-class restrictions exist to avoid; if a remainder is ever
needed, lifting a restriction is additive in a way that imposing one is not.

### Q2. Binary operators over two multi-valued expressions: cartesian, zip, or explicit?

Cartesian (jq today), zip
(vectorized, with broadcast), or neither by default with explicit `cross` and `zip`?

Vec concatenation specifically is decided, without touching the rest of this question:
`concat(vv: Vec<Vec<T>>) -> Vec<T>` flattens, as a named builtin rather than an overload of `+`
(see [named functions kept an open question open](research-log/named-functions-kept-an-open-question-open.md)).
The general question -- what `+`, or any other operator, means when both operands are Vecs --
is still open.

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

An exploration after [the streams decision](#decided-stream-is-the-effect-layer-typed)
committed to this shape without settling the open parts; it is recorded as
[ADR 0008](docs/adr/0008-stream-protocols-are-kleene-patterns.md). The load-bearing findings:
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

SETTLED: it does not need to. The two
 are the same operation with the multiplicity stored in different places, so `map` and
 `select` are not different kinds of thing. `map(f)` is `[ .[] | f ]`, meaning reflect,
 apply, reify, and `[...]` absorbs whatever cardinality the argument had. See the two-layer
 section. This was the longest-running untracked thread and is recorded here so it is not
 reopened by accident.

### Q12. On a type mismatch, does field access error, yield null, or something third?

SETTLED:
 something third. jq conflates missing with type error, so `null.a.b.c` yields `null` while
 `1 | .a` raises. Field access desugars to a lens returning three distinguishable outcomes: a
 value, a *specific* absence, and a *specific* error. See the field-access section.

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

Cranelift never auto-vectorizes, which
 matters because the whole vectorization argument here rests on handing a vectorizer loops it
 likes. It also emits nothing for the web, so a WebAssembly target would need a second
 unrelated backend. Cranelift wins decisively on build simplicity and compile speed, so using
 both, as rustc does, is a real option. LEANING toward LLVM for release builds.

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

Argued in [the admissible input set, and where batching comes from](#the-admissible-input-set-and-where-batching-comes-from) rather than
here, since it arrived with that material. The leaning is the trait law that operations commute
with reification, which is what makes a batch boundary invisible to a program.

### Q22. Are dense and masked vectors distinguishable in the type?

Argued in [the admissible input set, and where batching comes from](#the-admissible-input-set-and-where-batching-comes-from), where it
appears as the observation that a masked view and a dense buffer have different launch
preconditions. The same question as [what select returns](#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy), approached from
the layout side rather than the operator side.

### Q23. What primitive set is the standard library defined over?

Argued in [the primitive set cannot be fold and recursion](#the-primitive-set-cannot-be-fold-and-recursion). The leaning is the
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

Partly settled by [the enum decision](#decided-enums-nominal-and-json-native): closed nominal
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

SETTLED yes, by [the auto-matchers grilling](#construction-and-naming) (2026-08-29,
kantord/toylang#47): matchers are first-class, tagged, and or-composable, derived per type
with the capital name. The reasoning below is the argument that held.

LEANING yes. Giving `Result` itself boolean-algebra traits (`and`/`or`) would let a value read as
"successful" while still carrying an unread error, the same missing/error conflation [Q12](#q12-on-a-type-mismatch-does-field-access-error-yield-null-or-something-third) was
written to rule out. A `Matcher<T>` carries the yes-or-no algebra (`int() and even()`,
`int() or str()`, spelled with keywords rather than `&&`/`||` to stay clear of `|` as pipe);
running a matcher against a value is the separate step that produces a `Result`. The algebra
itself is not new -- it is [the regular-expressions-over-types idea](#q4-can-the-type-express-ordering-over-heterogeneous-streams)'s `Alt`/`Seq`/`Star`, applied at the
value level. See [Pattern matching is decoding](#pattern-matching-is-decoding).

### Q28. Does deep matching need cross-match unification of logic variables?

OPEN. `..` composed with a matcher already finds a shape anywhere in a tree without naming its
path, and `as` already binds one submatch to a name for reuse within the same arm. Neither needs
unification. What would: finding a node `A` and a separate node `B` elsewhere such that `B`
refers to `A`, which is full Prolog-style unification with backtracking over bindings, not a
bigger version of `as`. See [Pattern matching is decoding](#pattern-matching-is-decoding).

### Q29. What is the default discriminant convention for a derived enum codec?

SUPERSEDED by [the enum decision](#decided-enums-nominal-and-json-native), which dissolved the
question's premise: there is no derived codec choosing a representation, because the
representation *is* the value -- the single-key wrapper for payload variants, a bare string for
unit variants. The candidates as originally recorded: an explicit tag field (`{"kind":
"circle", "r": 1}`), a single-key wrapper (`{"circle": {"r": 1}}`), or matching purely on shape
when the variants are structurally distinct enough to allow it. The tag field lost knowingly --
it is what much wild data looks like, and typing such data as an enum now needs the deferred
codec layer; shape-matching cannot represent all-unit enums at all, though it survives as
`Opt`'s special form. See
[the constructor-should-be-free correction](#the-constructor-should-be-free-and-that-reverses-a-dependency-claimed-above).

### Q30. Do the base-functor generics double as parser combinators, across trees, strings, and streams?

LEANING yes. `Seq`, `Alt`, `Star`, and `Opt` are already in the document as [the regex-over-types algebra](#q4-can-the-type-express-ordering-over-heterogeneous-streams)
and as the shape [Pattern matching is decoding](#pattern-matching-is-decoding) builds `Matcher<T>` from; naming them as parser
combinators only makes the precedent explicit (Hutton and Meijer; Wadler; parsing with
derivatives). OPEN: whether this is one trait with implementations that differ by receiver (a
parsed tree needs no backtracking, a string needs an actual parsing engine), the same shape as
[`Field<K>`](#field-access-is-a-lens), and if so what law the implementations have to share. See
[One combinator algebra for trees, strings, and streams](#one-combinator-algebra-for-trees-strings-and-streams).

### Q31. Does a friendlier string-pattern language belong in the language, and what regex flavor does it extend to?

OPEN. A URL-route-style syntax with named, typed captures composing through the existing
`int(.)`-style codec syntax is one candidate, with Swift's `Regex` builder and route-pattern DSLs
such as Express's `path-to-regexp` as the closest prior art. [The arm-list's `//` semantics](#pattern-matching-is-decoding) already
commit any such language to ordered, PEG-style choice, which is compatible with PCRE/Perl-style
regex and not with POSIX leftmost-longest regex, so "extends to regular expressions" needs to
name which flavor. See
[One combinator algebra for trees, strings, and streams](#one-combinator-algebra-for-trees-strings-and-streams).

### Q32. Does the dimension model subsume the effect layer?

The two-layer section says multiplicity lives either in a value or in evaluation, and
[the one-way shift](#proposal-the-layer-shift-only-runs-one-way) narrows that to
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

[The enum decision](#decided-enums-nominal-and-json-native) has since answered every bullet for
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
level, and [the streams decision](#decided-stream-is-the-effect-layer-typed) gave the effect
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

The representation is [decided](#decided-float-is-javascripts-double): IEEE 754 binary64,
JavaScript's number. Everything observable about it is not, and each piece has to survive the
agreement harness, which checks bytes.

- **Printing.** Every backend must render the same double to the same text, and their defaults
  do not agree on shortest-roundtrip versus fixed formatting, or on `1e21`-style switchover
  points. The printer is currently the only specification of output format
  ([Q35](#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return)), which makes
  this a per-backend conformance rule to be stated by hand, the same lesson as
  [backends can agree and still be wrong](research-log/backends-can-agree-and-still-be-wrong.md).
- **NaN and Infinity.** IEEE produces both; JSON can spell neither. A language whose values
  are JSON-shaped either forbids them (a check on every producing operation), maps them to
  something at the boundary (jq-style lossiness, the kind this design usually refuses), or
  admits values its own output cannot carry.
- **Division by zero.** The Int rule says a zero divisor is the only way arithmetic fails.
  IEEE says `1.0 / 0.0` is `Infinity`, no failure at all. Keeping both means division's
  behavior depends on its operand type; unifying means overriding one standard or the other.

None of this blocks anything else, so it waits for `Float` to be forced by a real program the
way `inputs` and `jsonlines` were.

## Non-goals

- JavaScript semantic compatibility. Prototype chains, `this` binding, coercion, and array
  holes are explicitly not wanted.
- Being jq. Compatibility with a subset of jq's *semantics* is a starting point and a test
  corpus, not a constraint on the finished language.
