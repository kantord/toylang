# Trait interface: scoping the minimal checker mechanism

Research spike for [`trait-interface-research`](board.yaml), the spinoff of the
`overloading-and-utf8-lines` round's ruling
([`overloading-and-traits-design`](board-archive.yaml)). The ruling chose "declared
interface/trait mechanism (Rust/Swift-shaped)" over function overloading and over the
reserved-name convention ([the two alternatives surveyed
beforehand](language-precedents.md#framings-for-the-grilling-session-smallest-first)). This
spike scopes what that mechanism minimally requires of the checker, and whether it
truly needs user-facing generics to start. No toolchain was consulted or run;the
Rust and Swift halves are desk research against documented behavior. Nothing in
`src/` changes.

## The short version

The minimal mechanism -- a trait declaration, `impl` per concrete type, and
compile-time dispatch by `(trait, type)` lookup -- can start monomorphic, with exactly
one implicit type parameter, `Self`, borrowed from Rust and Swift. `Self` is not
"user-facing generics" in the sense the project has deferred
([`draft.md:2453`](../draft.md)): it is the same `Type::Param` substitution machinery
generic enums already ship with
([the generic-enum ratification](../draft.md#retired-2026-08-29-kantordtoylang62-generic-enums-shipped-with-optt-as-the-first),
the `unify`/`substitute` pair),just granted a reserved spelling. User-written `<T>`
parameters, generic impls over type constructors, generic functions with trait bounds,
and trait type parameters are all deferrable -- each is a convenience or a separate
feature, not a load-bearing part of the dispatch table. Dispatch resolves statically
from the concrete operand types the checker already has by the time an operator or call
is checked; no inference, no vtable, no function value, so
["functions are not values"](../docs/reference/syntax/functions.md) stays intact. The
result of a user trait dispatch is an ordinary call into the impl's body, which every
backend already emits; zero new `Kind`, zero backend changes, zero snapshot churn for
existing programs if the builtin types' impls are the checker's existing `binary`/`plus`
rules re-labeled rather than source-written.

## Where toylang's ad hoc polymorphism lives today

Three mechanisms, each closed to the program.

**The operator match is hard-coded over the left operand's type.** `binary`
(`src/check/mod.rs:2430`) and `plus` (`src/check/mod.rs:2535`) match on the left
operand's synthesised type:the two integer widths get arithmetic, `Str` and
`Vec` get concatenation for `+`, comparisons work on a same-typed pair and refuse
themselves on a `Vec` anywhere inside a composite. This is the "closed per-operator
match" the ruling names as what the trait mechanism supersedes:extending what
`+` means for a user-declared type is impossible today, because the match has no
user-populated table to consult.

**The builtin table is a fixed signature list.** `builtin()` (`src/check/mod.rs:514`)
holds the monomorphic, fixed-signature builtins. Above it, `call`
(`src/check/mod.rs:1798`) hand-special-cases the polymorphic ones -- `length`,
`tail`, `flatten`, `collect`, `fields`, `sort`, `reverse`, `sum`, `max`, `jsonlines` --
each with its own branch because their return types or argument shapes depend on the
element type, which no fixed signature can express. These are the second closed
mechanism:compiler-known, not user-extensible.



**Function calls dispatch by name, exactly one signature per name.** `signatures`
(`src/check/types.rs:213`) collects one `Sig` per function name; a duplicate name is
an error. There is no overloading in the function namespace, and per
[the method-call ruling](board-archive.yaml) the receiver-spelling call form
(`x:foo(y)`) that would eventually reach trait methods is ruled but not yet built. So
today the only name a call site can reach in the function namespace is a single-
signature function.

Three closed mechanisms, all resolved eagerly in the checker's first pass or at
expression-check time, none involving a value category beyond an ordinary function
call. That is the terrain the trait mechanism enters.



## The minimal trait story in Rust and Swift

Both languages, reduced to what the dispatch table needs, are the same three-part
shape. The differences that matter for toylang are the syntax and the way `Self` is
handled;the dispatch semantics are interchangeable.



**Rust.** A trait is a named collection of method signatures
([the reference](https://doc.rust-lang.org/book/ch10-02-traits.html)). The receiver
type is implicit and spelled `Self`: `trait Add { fn add(self, rhs: Self) -> Self }`.
An `impl Trait for Type` provides bodies, one per concrete type: `impl Add for i32`,
`impl Add for Str`. Each is an independent item;generic impls (`impl<T> Add for
Vec<T>`) are a separate, additive feature that requires type parameters. Dispatch
resolves statically by monomorphization:at a call site the compiler knows the concrete
type and emits a call to the matching impl's specialized instance. Operators desugar
to trait methods (`a + b` desugars to `<T as Add>::add(a, b)`) through `std::ops`;
the only special thing about an operator is its surface spelling. `dyn Trait`
opts into runtime dispatch via a vtable, which needs storing function pointers --
exactly the thing toylang's "functions are not values" rules out. The orphan rule
(impl must live in the same crate as the trait or the type) is vacuous for a single-file
program, everything is one crate. Two features Rust's own `Add` carries that the
minimal cut can simply omit:a defaulted `Rhs = Self` type parameter for mixed-type
addition, and associated types. toylang's "both sides must agree"
(`src/check/mod.rs:2516`) rule deliberately does not want mixed-width arithmetic, so
there is no `Rhs` to express, and omitting it shrinks the trait rather than hobbling
it.



**Swift.** A protocol is the same shape under another name:requirements are method
signatures, and `Self` appears in every one of them -- Swift requires a protocol to
mention `Self` (or an associated type), or it can only be used as `Any`). Types
conform through `extension Type: Protocol { ... }`;protocol extensions provide default
implementations the same way Rust's default trait methods do. Operators are declared
separately (`infix operator +`, binding precedence and associativity),and the protocol
requirement is a `static func`;the conformance then makes the operator meaningful for
that type. Dispatch devirtualizes statically whenever the concrete type is known, which
for toylang is always, since nothing in its checker introduces abstraction or
inference;`any Protocol` existentials are the runtime-dispatch escape hatch, again a
function-value story. A minimal monomorphic Swift:a protocol, per-type conformances,
no generics anywhere, is fully workable -- that is exactly what per-type retroactive
conformance does. The operator-declaration step is a registry toylang's fixed `BinOp`
table already is, so no new syntax there.



**What both share, stripped to the dispatch table.** A named interface with method
signatures written against `Self`;an impl table mapping `(trait, concrete type)` to
a body;compile-time dispatch by that pair. Neither language needs user-facing
generics for that core -- Rust's `impl Add for i32` and Swift's `extension Int: Add` compile
fine with no `<T>` anywhere. Both put generics to work the moment the mechanism leaves
the dispatch-table niche:bounds in generic functions, generic impls, associated
type constraints. So the design question for toylang is not "monomorphic or generic" as
a binary;it is "which side of that line does the first cut land on, and what does the
line cost".



## How it maps onto the existing checker

The mapping is unusually clean, because toylang's checker is already organized exactly
where the trait table needs to live.



**The eager first pass grows a fifth map.** `check` resolves every alias, enum,
and signature before any body is checked (`src/check/mod.rs:101-137`). A trait table
(`trait_map: HashMap<String, TraitDecl>`) and an impl table (`impl_map: HashMap<
(TraitName, Type), Impl>>`) land in that same pass, with the same whole-program
sequencing rule the existing resolution obeys
([the nanopass lesson](../research-log/merging-passes-turns-redundant-traversals-into-bugs.md)):
a trait must be resolved, and every impl collected, before any operator or call site
dispatches, so a call site can reach an impl declared later in the file. The
`Type` key already has structural equality (`src/ty.rs:331`);a hash impl is the one
missing piece if the impl table is to be a `HashMap` rather than a linear scan, which
is an implementation detail, not a design constraint.





**`Self` is the existing `Type::Param` machinery, granted a reserved spelling.** A
trait declaration's method signatures resolve in a context where `Self` maps to a
placeholder -- exactly how `enum Opt<T>`'s payloads resolve `T` to `Type::Param`
(`src/check/types.rs:126-133`). An impl registers `Self = <impl target>`;the
impl's bodies then check against the impl's own signatures, where `Self` is the target
type (via `substitute`, `src/ty.rs:379`),and at a dispatch site the checker matches
the call's concrete type against the trait's template signature via `unify`
(`src/check/mod.rs:737`),the same pattern-match-and-bind it already runs for generic
enum construction. No new type-theoretic concept:a trait method signature is a
template the same way an enum payload is.



**Dispatch is name resolution, not a new evaluation step.** A binary operator
`+` on a user type becomes:look up `(Add, lhs.ty)` in the impl table, check the
right operand against the impl's expected signature (the existing `expect` path),and
lower to an ordinary `Kind::Call` into the impl's body. The backends already emit
`Kind::Call` for every user function ([e.g.](src/emit_go.rs) `src/emit_go.rs:708`);a
trait impl is just another function, synthesized into `Program.funcs`, named by the
`(trait, type)` pair rather than by source. This is also exactly Rust's model:a
trait method impl is an ordinary function monomorphized per type. It is what keeps
"functions are not values" unharmed:dispatch is compile-time name resolution,the
same move variant-constructor resolution already makes (`src/check/mod.rs:1886`),when
a bare `circle{r: 1}` resolves through the enum registry rather than the function
namespace.



**Existing programs see nothing.** The builtin types' impls (`Add for Int`, `Add
for Str`, `Eq for Record`, ...) are the checker's existing `binary`/`plus` rules,
re-labeled as generated, implicitly-registered impls rather than source-written ones.
That keeps every currently-compiling program lowering to `Kind::Arith`/`Compare`/
`Concat` unchanged, so the corpus and snapshot tiers are untouched. It also gives
coherence for free, Rust-style:the builtin impls pre-exist, so `impl Add for Int`
is "already implemented", not an override. A user impl is reachable only for types
the builtin match declines, or for the operator/type pairs the design row decides to
open. The alternative -- source-written prelude impls for the builtin types -- would make
the builtin operators pass through emitted functions, changing every snapshot;the
research recommends against it.



**The one real gap:named trait methods need the receiver-spelling call form.**
Operators have their own spelling, so `+` dispatches without new syntax. Named trait
methods do not:a plain call `area(circle)` resolves in the function namespace, which is
name-keyed single-signature, and exactly what the ruling declined to overload. Rust
reaches trait methods through `c.area()` or fully-qualified paths;Swift through
receiver syntax. toylang's [colon call form
`x:foo(y)`](board-archive.yaml) is the ruled-but-unbuilt version of that. The
minimal trait mechanism therefore *needs* that call form (or an equivalent receiver
spelling)), which is a dependency on a feature the design row must weigh -- not a
blocker of the checker mechanism itself, since the dispatch table and the operator path
stand alone. The research's recommendation:scope the first cut to operators plus
the trait methods reachable through the already-ruled colon form, and treat any other
spelling as the design row's call.



## The monomorphic-vs-generic question, answered

The honest reading of "does this require user-facing generics" is:it depends on
which of three things "generics" is taken to mean, and only one of them is
required.



**`Self` is required, and it is not the deferred kind of generics.** A trait
declaration cannot name the types it will be implemented for, so it needs a
placeholder for "the receiver type". Rust and Swift both spell that `Self`,and both
require it in a way that the protocol/trait is useless without it. toylang's generic
enum machinery already has this exact placeholder (`Type::Param`)and the substitution to
apply it;granting `Self` a reserved spelling is a grammar change, not a type-system
one. A reader who counts `Self` as "user-facing generics" should note it is the
generic *enum* kind already shipped
([`draft.md:2456`](../draft.md#retired-2026-08-29-kantordtoylang62-generic-enums-shipped-with-optt-as-the-first),not the deferred generic *function* kind.



**Generic impls over type constructors are a convenience, not a requirement.** Rust's
`impl<T> Add for Vec<T>` is one impl covering every element type;toylang's first cut
could instead require `impl Add for Vec<Int>`, `impl Add for Vec<Str>`, ... per
concrete instantiation. This is verbose but total,and it works because toylang's type
system is closed:every type a program can name is the builtins plus its own
declarations, so the set of instantiations a program needs impls for is finite and
known to the compiler. The cost is that the impl *does not compose*:a `sum`-style
function wanting "any additive element type" still cannot be written once. That is
exactly the generic-function gap below, arriving early rather than late.



**Generic functions with trait bounds are the only thing that genuinely requires
generics,and the minimal mechanism does not need them.** A function whose signature
names a type parameter (`fn sum<T: Add>(...)`) is what forces user-facing generics. Every
expression the checker types today synthesizes or checks against a concrete
type, with no type variables in flight, so a call site never has a "T" to resolve a
bound against. The polymorphic-over-element builtins (`length`, `flatten`, `sum`, ...)
are exactly the functions a generic implementation would replace,and each can keep
its current closed branch until generic functions land. A trait mechanism that could
also *express* them is the "trait as the constraint mechanism for generic code" story,
which is Rust's and Swift's actual center of gravity,but it is a separate
feature on the board, not part of the dispatch table.



**Recommendation:start monomorphic,with `Self` as the only parameter.** Build
the trait declaration (`trait Add { fn add(lhs: Self, rhs: Self) -> Self }`),
per-concrete-type impls,and static dispatch by `(trait, type)`, now. This is the
smallest mechanism that genuinely supersedes the closed per-operator match,and lets
user types open operators and reach named trait methods. Defer, in order:generic
functions with bounds (a signature feature,the next real generics step after generic
enums);generic impls over type constructors (a convenience once generic functions
land, since the bound form subsumes them);trait type parameters (`trait Field<K>`,
the `Add<Rhs>` shape)and associated types(needed for the trait-law stories
[`draft.md:888`](../draft.md#the-law-is-that-the-operation-commutes-with-reification)),and
`dyn`/existentials(already ruled out forever by functions-as-not-values). The
declaration syntax chosen now should be `Self`-based so those later additions extend
the signature vocabulary rather than reshaping the declaration, which is exactly what
Rust did with `Rhs = Self` and what Swift did with protocol extensions.



**The one cost worth naming, so the design row adopts it with open eyes:** until
generic functions land, a *trait* in toylang is a dispatch table, not a
constraint. It opens operators to user types and gives a name to a set of impls,
but it cannot yet appear in a signature to say "any `Add`". That is closer to
overloading than to Rust's trait system -- which is fine,because the ruling asked for ad
hoc polymorphism, which is precisely the dispatch-table half of what traits do. The
"trait as law" story (`draft.md:888-911`,batch invariance) stays unimplemented
until generics land,and should be recorded as such, not quietly dropped. A trait
without a stated law is only overloading ([`draft.md:903`](../draft.md)) --the first
cut should say plainly that it is the overloading half,the law half is deferred, not
pretended away.



## The scoped mechanism

Concrete shape, at the level the checker needs, with the syntax left as
candidates for the design row to weigh (this spike scopes mechanism, not spelling).

```toml
# declaration (Rust spelling shown; Swift's `protocol` is the alternative)
trait Add {
    fn add(lhs: Self, rhs: Self) -> Self
}

# impl, one per concrete type
impl Add for Circle {

    fn add(lhs: Self, rhs: Self) -> Self = {x: lhs.x + rhs.x, y: lhs.y + rhs.y}
}

# use: an operator, unchanged surface syntax
a + b

# a named method, through the ruled-but-unbuilt colon call form
c:area()
```

Well-formedness the checker would enforce, all reusing existing rules:

- A trait name follows the casing rule (capitalized, like an enum;it creates no
  value, so the "capital means type" rule applies),and cannot collide with a
  builtin, alias, enum, or another trait name. A trait may be `pub`, with the
  same per-file visibility tracking enums already carry (`src/ast.rs:132`).
- Every impl names an existing trait and a concrete resolved type;the type's
  instantiation is what `Self` substitutes to. Two impls for the same `(trait,
  type)` pair are an error,the duplicate-definition rule extended;the builtin
  types' generated impls pre-exist, so an impl for one is "already implemented".
- A trait method signature must mention `Self`, at least once;otherwise it names
  nothing per-impl and dispatch has no meaning to bind. This is Swift's
  requirement, stated as a toylang rule. Every trait method is unary (the
  language's universal arity),with its one parameter `Self`-typed or an
  ordinary type mentioning `Self`;multi-parameter operations take a record
  argument the way every call does.
- Dispatch resolves by the left operand's concrete type for operators (the
  existing `binary` order)and by the receiver's type for the colon call form.



Checker changes, in the order a build would land them:

1. Grammar:`trait` and `impl` keywords, a `TraitDecl` and `ImplDecl` node
    each, slots in `File`/`Module` beside enums. A `Self` type spelling in
    type annotations, resolving to a reserved `Type::Param` name.

2. The eager pass:collect trait declarations and impls into the two tables,
    resolving `Self`-bearing signatures against each impl's target via the
    existing `resolve`/`substitute` machinery, with the duplicate checks above.
3. Dispatch:in `binary`/`plus`, after the builtin match declines, consultthe
    `(trait, left.ty)` table, check the right operand against the impl's expected
    signature,and lower to `Kind::Call` into a synthesized `Program.funcs` entry.

    In `call`, after the function namespace declines, resolve a colon-form receiver
    call through the same table. No new `Kind`, no backend changes.





## Out of scope, recorded so the design row does not inherit them by accident

- Generic functions, generic impls, trait type parameters, associated types,
  and anything that puts a trait name in a signature. A generic *enum* is already
  precedent and untouched by this mechanism.
- The trait-law story (`batch invariance`, `Field<K>`'s law, codec families):the
  shape admits it later,the checker cannot verify a law,and it is not part of
  the dispatch table. Recorded as deferred, not dropped.
- Makingthe polymorphic builtins (`length`, `sum`, `flatten`, ...) into trait
  instances:that is the generic-functions landing, not the monomorphic cut.



## Open questions for the trait-interface-design row

- Whether operators' generated builtin impls live as checker rules (recommended,
  zero churn) or as source-written prelude impls (uniform, costly).
- Whether `+`'s current "both sides must agree" width rule survives unchanged as
  the builtin `Add` impl's signature, or whether the impl's expected-signature check
  relaxes it anywhere.
- The colon call form (`x:foo(y)`,the ruled-but-unbuilt gh:119 spelling) as
  the trait-method receiver, versus a different receiver spelling;and whether
  trait methods also gain a plain-name path when exactly one impl exists (the
  constructor-resolution precedent,`src/check/mod.rs:1886`).
- Whether `Self` is a reserved word, a type-level spelling like `Record`, or a
  special case of the parameter machinery --and what error names it when written
  outside a trait or impl.
- What a trait *is* categorically:a type (so it can be named in `pub`,
  shared,and later in signatures),or a second-class declaration the way the
  parser treats `input`. The casing rule and the future bound story both push
  toward "a type";the research recommends that,but flags it as the design row's
  call.
- Whether the builtin composite rules (`Eq` structural, stopping at a `Vec`) remain
  hard-coded generated impls, or become per-type impls a program can see and
  extend -- the line that decides how much of `binary` migrates into the impl table.
.


The receiver-spelling dependency on the colon call form (ruled gh:119, unbuilt)

is the one feature the minimal mechanism cannot avoid;every other piece of the
scoped shape above is checker-internal and can land without new surface syntax.
.