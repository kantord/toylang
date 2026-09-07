# toylang design draft

**Status: exploratory. Everything here is provisional.** This is a thinking document, not a
specification. Syntax is illustrative; several core decisions are still open (see the end).

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
[unwrap](docs/reference/operators/unwrap.md) is the existing idiom for "give me the value or propagate the failure," and
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
is the missing/error conflation [Q12](plans/questions.md#q12-on-a-type-mismatch-does-field-access-error-yield-null-or-something-third) was written to rule out. A matcher's `and` / `or` combine the yes-or-no
question of whether something matches; *running* a matcher against a value is the separate step
that produces the `Result`. REJECTED: giving `Result` the algebra directly.

The algebra is not new, either. [The regular-expressions-over-types idea](plans/questions.md#q4-can-the-type-express-ordering-over-heterogeneous-streams) already proposes `Alt<A,B>`, `Seq`,
and `Star` as the primitives for describing a sequence of types. `int() or str()` is `Alt`, a
record pattern's fields in sequence is `Seq`, and `any()*`-style repetition over a `Vec` is
`Star`. This is that algebra's value-level instance rather than a second one.

One distinction the syntax hides and the compiler must not: matching a value whose type is
already closed -- an enum you already have an instance of -- is total, checkable for
exhaustiveness at compile time, no `Result` anywhere. Matching untyped `Json` is partial, since a
shape nobody wrote a case for is a live possibility, so it needs `any()` or a `Result`. The two
compile to different things behind one shared arm syntax.

`.` shadowing the wider value inside a matched arm is correct, and it is [the same rule the document
already has](docs/reference/syntax/functions.md) for block-scoped `.`:
`map(.name)` rebinds `.` to each element already, so a matched arm rebinding `.` to the narrowed
type is that rule applying once more, not a new one. Inside `int(.) -> ...`, `.` has type `Int`,
not `Json`, and the wider `Json` is not reachable by name.

Convenient unwrap-to-error already has its syntax: `.name!`, from [unwrap](docs/reference/operators/unwrap.md),
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
by which types the codec sits between -- the same way the [lens trait](plans/questions.md#q40-is-a-fieldk-lens-trait-part-of-the-design) would pick an
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

This does raise the stakes on [whether `..` promises order](plans/questions.md#q7-does--promise-depth-first-order-or-only-the-set-of-nodes). Before, the order it walks in was
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
out of things that already have one, the same structural composition the [lens trait](plans/questions.md#q40-is-a-fieldk-lens-trait-part-of-the-design) would use
per field. Most declared types are exactly this case, so deriving the pair mechanically, rather
than requiring it be hand-written, is the right default. Precedent: Rust's
`#[derive(Serialize, Deserialize)]`, Haskell's generic `aeson` deriving, Scala's circe
derivation.

One place this does not fall out for free: an enum needs a **discriminant convention**, some
agreed way for the `Json` to say which variant it is (`{"kind": "circle", "r": 1}` versus
`{"circle": {"r": 1}}` versus matching on shape alone). Struct fields compose without asking this
question; variants cannot avoid it. Tracked as [Q29](plans/questions.md#q29-what-is-the-default-discriminant-convention-for-a-derived-enum-codec).

A JSON Schema document is a third projection of the same structural information, and it is
cheaper than the codec rather than harder: it describes the shape without running any decode
logic, so it falls out of whatever internal representation the derived matcher already builds --
the same representation [the regular-expressions-over-types idea](plans/questions.md#q4-can-the-type-express-ordering-over-heterogeneous-streams) assigns each type (`Alt` for a
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
Spiewak) is the same idea again, and it is also what [the regex-over-types entry](plans/questions.md#q4-can-the-type-express-ordering-over-heterogeneous-streams) is already
reaching for with "unpacking one item is the derivative of the pattern." Naming it does real
work: this design does not have to invent parsing theory, only decide which known combinator set
to adopt.

One thing the name does not give away for free: a `Json` tree is already fully parsed, so
recursing through it needs no backtracking and no notion of position, while a string or byte
stream needs an actual parsing engine underneath, because "does `a*` match here" can require
trying more than one length before the surrounding pattern succeeds. So this is one algebra with
at least two implementations, the same shape the [lens trait](plans/questions.md#q40-is-a-fieldk-lens-trait-part-of-the-design) would have for indexable versus
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

[The vectorizability question](plans/questions.md#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization) was argued on the grounds that cardinality and vectorizability are *orthogonal*: `select`
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

**[The blocking-operator question](plans/questions.md#q20-how-are-blocking-operators-sort-group_by-joins-classified) then answers itself.** The blocking operators are exactly those with no lawful stream
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
to be distinguishable in the type, since they have different launch preconditions. This is [the select-result question](plans/questions.md#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy)
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
[Q1](plans/questions.md#q1-streams-first-class-values-or-evaluation-level-multiplicity).

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

## What the prototype showed

A working compiler exists: `plans/` has the build order, `research-log/` has the findings, and
the language it accepts is the one described above minus the effect layer, object construction,
and everything listed under prototype 1's exclusions. It runs on three backends -- Lua,
JavaScript, and native through LLVM -- and a corpus of 22 programs is checked to produce
identical output on all three, with disagreement between backends counted as its own kind of
failure.

That produced evidence for questions that had been argued rather than tested. Recorded here as
what happened, not as verdicts; the statuses in [the open questions table](plans/questions.md) are
still yours to move.

### Stream lowering does not block a backend that does not stream

[The stream-lowering question](plans/questions.md#q5-stream-lowering-strategy-across-the-three-backends) was recorded as blocking all backend work, and its
detail said the strategy must be decided before any backend is written. Three backends are
written and it was never touched.

The reason is that prototype 1 has no effect layer, so every program has statically known extent
and lowers to a counted loop on any target, including one with neither coroutines nor
generators. It blocks *streaming* backend work, which is a much narrower claim, and it means the
window in which backends are cheap is exactly the window before streams exist. That row is
corrected rather than proposed, since it is a fact about what the repository now does.

### The one-way layer shift held, and it has a price

Prototype 1 implemented no value-to-effect shifter at all, taking
[the one-way shift proposal](plans/questions.md#q13-does-the-layer-shift-run-only-one-way-with-no-value-to-effect-operator) at its word to see what would break. Nothing needed one,
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

[Whether vectorizability is visible in the type](plans/questions.md#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization) gains an argument for staying silent.
Under struct-of-arrays, `select` binds `.` to a position rather than a value, so `.age >= 18`
compiles to `ages[i]` and nothing materialises an element. The vectorizable form is what falls
out of compiling the obvious thing against that layout: no pass recovered it, and nothing in the
type had to declare it.

Not decisive, but it is a data point against paying for a second effect to report something the
layout already provides.

### Masking now has an implementation to argue with

[What select returns](plans/questions.md#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy) and
[whether dense and masked vectors are distinguishable](plans/questions.md#q22-are-dense-and-masked-vectors-distinguishable-in-the-type) were open in the abstract.
`select` is a copy today: it builds a mask and then compacts every column with the same surviving
indices. Under struct-of-arrays a masked view is visibly the cheaper option, because compaction
is the only part that touches element data at all. Still open, but open against something
measurable. See
[SoA is cheap until something wants a whole element](research-log/soa-is-cheap-until-something-wants-a-whole-element.md).

### The native backend is built

[The backend choice](plans/questions.md#q15-backend-llvm-via-inkwell-cranelift-or-both) is demonstrated rather than leaning. LLVM through inkwell,
against LLVM 22.1. Native output is an object file plus a linked C runtime, since LLVM does not
link, and string concatenation, integer formatting and JSON parsing all want C rather than
hand-written IR.

### Three string representations now disagree in a specific place

[The string representation question](plans/questions.md#q16-string-representation-given-wtf-16-on-the-js-target) is concrete. Lua holds bytes, JavaScript holds
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
- The payoff is exactly [Q10](plans/questions.md#q10-is-uniqueness-analysis-in-scope-for-deciding-when-a-lens-materializes)'s
  question answered from the other side: native's vector implementation (and other targets)
  gets honest in-place mutation, and the when-does-a-lens-materialize question gains its
  mechanism. Related: [Q14](plans/questions.md#q14-does-select-return-a-masked-view-a-selection-vector-or-a-copy)
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
own element always ends up laid out like its declared position expects. This originally reached
only a record's own fields; a Vec or Stream whose element arrived already built with a
different order than the container's declared element type stayed unreconciled, corrupting the
native backend's struct-of-arrays reads silently rather than refusing. Closed for Vec and
Stream by an actual per-element transform (a real `map` rebuilding every element, not a local
relabelling) in kantord/toylang#64. Opt is still open: nothing in the language can build a
"present" Opt value outside the handful of forms that produce one directly, so there is no
literal for the checker to rebuild into yet.
