# Closures and first-class functions: what would make currying fall out

Groundwork for gh:119 (prelude-partials-and-mutation round, Q3). The maintainer's ruling:
both struct-like and array-like partial application are needed, chosen by the input value's
shape, and true currying should fall out for free once closures/functions are first-class
rather than being implemented as its own mechanism. This surveys how Rust, OCaml, Haskell, and
Scala make functions first-class, what "first-class" has to mean in toylang's type system for
that claim to hold, and how the two partial-application shapes land on it.

## What toylang has today

There is no arrow type. `Type` (src/ty.rs:14) is `Str`/`Int`/`Int64`/`Float`/`Bool`/`Char`/
`Vec`/`Record`/`Stream`/`Sink`/`Enum`/`Param`, and a function signature is `Sig`
(src/ty.rs:383), a separate four-line `{param: Option<Type>, ret: Type}` struct carrying a
one-parameter-maximum comment. Functions are name-keyed `Kind::Call`s (src/tir.rs:76) inlined
by every backend, never values; a call names a function and the backend emits that name as a
native function. There is no `lambda`/`Closure` node in the AST or TIR, and nothing turns a
function into something `map` or `select` could receive except by name.

One fact does the load-bearing work: arguments already travel as a record for multi-argument
functions (Q33, questions.md:402). The language is one unary function per name, several
arguments as one record. That is already the curried shape in miniature. What is missing is
for that function to be a value.

## What "first-class" has to mean here

For currying to fall out rather than be built, two things must be true.

1. **An arrow type exists in the type grammar.** Currying is the identity
   `(a, b) -> c` is `a -> (b -> c)`. toylang has no tuple product for the left side, but it
   has the record: `{a: A, b: B} -> C` is the function-of-record form it already uses. The
   arrow type is what lets "a function whose result is a function" have a type, and that is
   the whole of the maintainer's definition ("a true curried function would be a function
   that returns another function"). Q33 already names this as the real cost: the residual of
   a partial application is a function, so it needs one to have a type.

2. **Functions are values.** An expression can denote a function, be bound, passed, and
   returned. Today a call is a name and the backend inlines the body; a function cannot be a
   value in a `Vec` or a record, and `map` takes a name, not an expression. First-class means
   the closure is a run-time thing, not a name the emitter knows.

The second half is really *closures*, not just function references. `join {with: ", ", ...}`
returns a function that remembers `with` -- that is environment capture, not a pointer to a
fixed body. So "first-class functions" in toylang means closures, and the type-system
question is what a closure may capture.

## The interaction with the effect layer: where "free" stops being free

This is the part of the "falls out for free" claim that has to be checked, because toylang is
not a pure value language. Streams are not values (ADR 0001): they exist only at effect-layer
multiplicity, consumed exactly once per binding, and the checker tracks that consumption
(linearity.rs). A closure is a value that can be passed around and called any number of times.
The two collide twice.

- **A closure is a value holding a non-value.** The "no stream as value" rule (ty.rs:54) has
  to say what a closure over a stream is. A curried stream-producing function, applied once,
  hands back a function; applying *that* later produces the stream at that later point, which
  is fine, but only as long as the closure never *stores* a stream. Currying `lines` is the
  first thing a user reaches for, and it is exactly the case Q33 calls out when it says first-
  class functions are blocked "which nothing else currently needs."
- **Linearity generalizes badly.** Each stream is consumed exactly once. A closure that may be
  called zero or more times over a captured stream either breaks single-consumption or must
  capture only effect-free values. The linearity checker reasons per binding about one use; a
  function value turns "this call runs once" into "an unknown number of calls run." `Sink` has
  the same problem one step further out (a sink is not even a value).

The honest read: "free" is true only for the value layer. The effect layer makes first-class
functions a real design decision rather than a freebie, because closures and streams both
touch "how many times does this run."

## The survey: how four languages make it free

**Haskell** is the reference model. Functions are curried by default: `add x y = x + y` has
type `Int -> Int -> Int`, application is juxtaposition, and `add 1` is a full application
whose result happens to be a function. There is no partial-application mechanism at all --
partial application is just application that returns a function, and it exists because the
arrow type makes a multi-argument function a nested unary one at definition time. The cost is
that every multi-argument call is sugar over nested application.

**OCaml** has the same default currying for positional arguments, and adds labeled arguments
(`~label:` with `?label` optionals), which give struct-like application: labels may be given
in any order, and an application missing labels returns a function over the rest. So OCaml
already has both mechanisms in one language -- positional (array-like) by currying, labeled
(struct-like) by a record of labels -- and they coexist because the type system keeps the two
kinds of argument apart.

**Scala** is the cautionary case. Methods are not values; `f _` (eta-expansion) lifts one, and
multiple parameter lists (`def add(x)(y)`) give curried application `add(1)(2)`. The `_`
placeholder does positional partial application (`f(1, _)`), and named arguments give the
struct-like side. The lesson is the cost of not committing: lifting a method to a function is
explicit, currying is opt-in via parameter lists, and partial application needs a placeholder
convention because the language is not curried by default.

**Rust** is the other cautionary case, and the one closest to toylang's situation. Functions
are first-class through fn pointers (`fn(i32) -> i32`) and closures (`impl Fn(...)`), but
there is no currying: partial application is a closure capturing (`move |x| f(1, x)`) or an
external crate built on tuples. The relevant lesson is the `Fn`/`FnMut`/`FnOnce` split -- a
closure's type records how it captures and whether calling it mutates or consumes. That
three-way distinction is what a closure type system needs once environment capture exists, and
it is the direct antecedent of the linearity interaction above.

What the four agree on: none builds partial application as its own feature. It is the
combination of an arrow type and functions as values, plus, in the curried languages,
multi-argument functions defined as nested unary ones. Partial application only becomes "a
mechanism" in the uncurried languages (Rust, Scala's method lifting), where a bespoke step is
needed to turn a multi-argument call into a function over fewer arguments.

## Struct-like vs array-like, and the input shape

The maintainer ruled both are needed, chosen by the input value's shape. The survey gives that
shape test a name: the two mechanisms are the two ways a language's functions can take several
arguments.

- **Array-like (positional).** The arguments are a positional list. `five_plus [1, 2]`
  supplies a prefix of the positions; the residual is a function over the rest. This is the
  curried/positional model -- the input shape is a sequence, and "partial" means a proper
  prefix of it.
- **Struct-like (named).** The arguments are a record. `join {with: ", ", ...}` supplies a
  subset of the fields; the residual is a function over the complement (Q33's field
  subtraction). The input shape is a record, and "partial" means a proper subset of the fields.

In toylang the two collapse onto one axis, because functions are unary and "several arguments"
already means a record. The array-like form only has somewhere to live if the language accepts
a positional sequence as a function's argument list -- which is exactly the `five_plus [1, 2]`
case the maintainer put on the table. So the shape test is literal: a function whose single
parameter is a record is struct-like (subtract fields); a function applied to a positional
sequence is array-like (drop a prefix). The maintainer's "function will still always only have
one parameter" keeps this coherent: there is never a true multi-argument function, only unary
ones over different input shapes, and each shape gets the partial-application rule that
matches it.

The interaction with first-class functions: both rules produce a residual function, which
needs the arrow type to typecheck, and neither needs bespoke partial-application machinery
once the arrow exists and application returns a function value. The struct-like rule is field
subtraction on a record already in the type system (Q33); the array-like rule is the curried
nested-unary shape the curried languages use.

## Open for the follow-up design row

The question partial-application-system-design (board.yaml:259) will grill: whether the two
forms are both real, or whether array-like *is* what currying is (drop a prefix, get a
function) and struct-like *is* what record arguments are (subtract fields, get a function),
with "chosen by input shape" meaning only the spelling differs while the underlying mechanism
-- application returns a function -- is one. The maintainer's own framing leans toward the
latter reading.

Two things the design row should verify rather than trust here. First, whether the array-like
form can be expressed on the jq backend: jq's `map`/`select` take filter expressions rather
than function values, so "function as a value" may have no jq spelling and the array-like case
would be the first to force one. Second, where the arrow type lands in `Type` -- it must be
excluded from the same positions as `Sink`/`Stream` (a `Vec<...>` of functions, a record field
holding a closure, a closure as an enum payload) unless and until the effect-layer interaction
above is settled, since each of those positions is where a stream could get captured as a
value.
