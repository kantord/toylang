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

## Non-goals

- JavaScript semantic compatibility. Prototype chains, `this` binding, coercion, and array
  holes are explicitly not wanted.
- Being jq. Compatibility with a subset of jq's *semantics* is a starting point and a test
  corpus, not a constraint on the finished language.
