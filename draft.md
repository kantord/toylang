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

The starting point is jaq's front end, meaning its parser, its 28-node `Term` IR, its value
model, and its test corpus. What gets replaced is the interpreter, a tree-walking evaluator
that allocates a boxed iterator per step. That allocation is the performance ceiling a compiler
removes.

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

1. **Are streams first-class values, or evaluation-level multiplicity?** jq has no stream
   *value*: `Val` has no stream case, and streams exist only during evaluation. That keeps
   values finite and acyclic, and lets streams compile to loops rather than heap-allocated
   iterators. But the base-functor formulation above implies a *remainder*, which is a value.
   Possible resolution: coinductive in the type system, erased by the compiler when the stream
   provably does not escape, with an acknowledged cliff when it does.
2. **Binary operators over two multi-valued expressions.** Cartesian (jq today), zip
   (vectorized, with broadcast), or neither by default with explicit `cross` and `zip`?
3. **What symbol replaces `=`** for the product-forming assignment?
4. **Ordering guarantees over heterogeneous streams.** If a stream is "some `A`s, then some
   `B`s", can the type say so? One approach is *regular expressions over types*, the same
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

## Non-goals

- JavaScript semantic compatibility. Prototype chains, `this` binding, coercion, and array
  holes are explicitly not wanted.
- Being jq. Compatibility with a subset of jq's *semantics* is a starting point and a test
  corpus, not a constraint on the finished language.
