# Language precedents: field order, and what dissolves the flatten family

Issue #36, from two maintainer questions ("what does Rust do here? how do other languages deal
with this?"). Two unrelated halves, feeding two different decide sessions:
[`record-order-strictness`](board.yaml) (issue #24) and
[`overloading-and-traits-design`](board.yaml). Framings for those sessions to weigh, not designs
for either.

## Field order: declaration, construction, and wire order

Issue #24 asks whether `{a: Str, b: Int}` and `{b: Int, a: Str}` are one record type or two, now
that field order lives in the type at all (`docs/reference/types/record.md`, the coordinator
ratification note at its top). Five precedents, each keeping a different set of these three
things separate: the order a type *declares* its fields, the order a *construction site* spells
them, and the order they appear *on the wire*.

**Rust.** Structs are nominal: two structs with identical fields but different names are
unrelated types, no exception. But declaration order and construction order are already split.
`struct P { a: i32, b: i32 }` fixes memory layout (modulo `repr` and layout randomization) to
declaration order, yet `P { b: 2, a: 1 }` is a legal construction of that exact type -- named-field
struct expressions in Rust have always been order-free, and so is pattern matching against them.
There is no second type produced by writing the fields in the other order; there is no second
type to produce, because struct identity is the name, not the spelling.

**TypeScript.** Structural, not nominal: `{a: string, b: number}` and `{b: number, a: string}`
are the same type, interchangeable everywhere, because object-type identity is a set of
name-to-type pairs with no positional component at all. TypeScript does have an order-sensitive
product type -- the tuple, `[string, number]` -- but it is a distinct type former from the object
type, never the same one under two readings. That separation is the sharp point of contrast with
toylang: no mainstream language fuses "named fields" and "position matters" into one type former
the way a toylang record now does, which is exactly why this question has no direct precedent to
inherit an answer from. Object types answer construction and lookup by name; tuples answer it by
position; nothing answers it by both at once.

**OCaml.** Records are nominal, scoped to the type declaration (`type point = {x: int; y: int}`),
but construction is order-free through labels: `{y = 2; x = 1}` builds a `point` the same as
`{x = 1; y = 2}`. When a label is ambiguous because more than one record type in scope declares a
field of that name, OCaml resolves it structurally by picking the most recently declared type
with that label (overridable with an explicit type annotation) -- disambiguation runs on the set
of labels present, never on the order they were written in.

**Go.** Structs are nominal, and Go is the one precedent where declaration order has real teeth:
two struct types are identical, for the purposes of assignability and conversion between named
types, only if they list the same field names, types, and tags in the same sequence (Go
spec, "Struct types" / "Type identity"). A positional literal, `P{1, 2}`, must supply fields in
that exact order. But a *keyed* literal, `P{b: 2, a: 1}`, is order-free the same as Rust and
OCaml, and it constructs the one type `P` -- Go never treats the two keyed spellings as different
types, only positional literals and cross-type conversion care about sequence.

**serde and wire formats.** The cleanest split of all three axes. `#[derive(Serialize)]` writes
struct fields in declaration order by default. Reading them back is format-dependent: a
self-describing format (JSON) matches incoming keys by name through `visit_map`, so wire order
is discarded on the way in and declaration order governs only what gets printed. A
non-self-describing format (bincode, and MessagePack's compact mode) has no names on the wire at
all -- fields are read positionally through `visit_seq`, so declaration order *is* the wire
format, and reordering fields is a breaking change to the encoding. This is the one precedent
where "the fields agree, but the order does not" is a real, familiar failure: it is exactly what
happens when a bincode reader and writer disagree on field order across a version skew.

**What this says about the two readings issue #24 weighs.** In every precedent, order is free at
the point a value is *built or matched by name* (Rust and OCaml struct/record expressions, Go
keyed literals, JSON deserialization); where order does bind, it governs something else entirely
-- memory layout (Rust), a different type former (TypeScript's tuple), cross-type convertibility
(Go), or the physical bytes of a non-self-describing wire format (bincode). None of the five
treats two name-complete, differently-ordered spellings of the same field set as two distinct
types at the point of use, the way toylang's current strict reading does. That does not make the
strict reading wrong -- `docs/reference/types/record.md`'s note gives its own reason, that
everything downstream (TIR layout, backend columns, the printers) already keys on position, which
is an implementation argument, not a precedent one -- but it does mean draft.md's framing of it as
"the conservative continuation" is a continuation of this codebase's existing positional
plumbing, not of how any of these languages actually treat record identity.

The other reading issue #24 names, order-insensitive equality with re-keying at every meet, has
one precedent that does something similar: JSON deserialization re-keys by name once, at exactly
one boundary (the read). What issue #24's loose alternative proposes is a materially larger
version of that same operation, repeated at every internal join point a value can reach (call
args, Vec elements, if/else, match arms, fn returns) rather than once at the program's edge --
worth naming as the actual size of the "loose" option, since none of the five precedents pay that
cost anywhere.

## The flatten/concat family and what dissolves it

The second half feeds `overloading-and-traits-design`, which exists to settle whether Vec
concatenation ever gets an operator, and if so, through what general mechanism -- since today
toylang's `+` overload and its `concat` builtin are both one-off, hand-written special cases
rather than instances of anything general.

**Where toylang stands today.** `+` is checked by one match on the left operand's type
(`src/check.rs:1918-1943`): `Int` gets arithmetic, `Str` gets concatenation, anything else is a
type error. Vec is deliberately excluded -- `plans/language-oddities.md:236-248` records that an
overload would prejudge the open Q2 (whether a binary operator over two multi-valued things means
cartesian, zip, or something explicit; `draft.md:2527`). Flattening a `Vec<Vec<T>>` instead goes
through a named builtin, `concat` (`src/check.rs:1355-1370`), which is unary where every other
language's `concat` is binary -- the comment there says plainly it exists as a named function
"so it does not decide Q2." `concat` sits alongside three other builtins -- `extent`, `tail`,
`collect` (`src/check.rs:1317-1372`) -- each hand-written as its own checker branch rather than
one generic function, because toylang's first cut is monomorphic: no user-facing type parameters
(`draft.md:2456`, "generics' first real customer is `Result<T, E>`"). Four different builtins,
four different ad hoc polymorphism sites, one per function, is what "no generics" costs today.

**Three precedents for how a language buys ad hoc polymorphism instead of one-off cases.**

- *Rust traits.* A named interface (`trait Add { fn add(self, rhs: Self) -> Self; }`, and the
  standard library literally implements operator overloading this way through `std::ops`),
  `impl Trait for Type` per concrete type, optional default methods, dispatch resolved statically
  by monomorphization unless the trait is used as `dyn Trait`, which needs a vtable and an
  indirect call at runtime.
- *Swift protocols.* The same shape under a different name -- a protocol is declared, types
  conform to it, and protocol extensions add default implementations the same way Rust's default
  trait methods do. Dynamic conformance (`any Protocol`, an existential box) is Swift's
  equivalent of `dyn Trait`.
- *Haskell type classes.* The historical origin of the idea (Wadler and Blott's ad hoc
  polymorphism), and the one whose implementation is most exposed: `(+) :: Num a => a -> a -> a`
  compiles to an ordinary function that takes one extra, invisible argument -- a record of
  functions (the `Num` "dictionary") -- resolved by the compiler from the call site's inferred
  type and passed in like any other value. Rust's and Swift's dispatch mechanisms are compiled
  forms of the same idea; Haskell is just the version where the dictionary is visible as data.

**Why none of the three ports over directly.** All three ultimately need a function to be a value
in hand at the call site -- a vtable entry, a monomorphized symbol resolved at compile time but
still conceptually "the function for this type," or Haskell's explicit dictionary of function
values. `docs/reference/syntax/functions.md` states the constraint that rules this out flatly: "A
function is not a value -- it cannot be stored, passed, or returned." There is no slot to put a
method table into, dynamic or static. Separately, none of the three can be adopted in its usual
shape without also adopting user-facing generics (a trait bound is a constraint on a type
parameter, and toylang has deferred type parameters entirely, `draft.md:2456`), so importing any
of them now would be answering a bigger, currently-parked question as a side effect.

**Framings for the grilling session, smallest first.**

1. *Name what already exists, add nothing.* The closed match on operand type
   (`src/check.rs:1918-1943`) and the four one-off polymorphic builtins are already toylang's
   entire ad hoc polymorphism mechanism -- total, compiler-known, not user-extensible. This is
   free, and it is what `plans/language-oddities.md:248` already argues resolves the concat
   naming/arity asymmetry without touching Q2: rename or re-arify `concat` and stop there.
2. *A reserved-name convention, resolved by the compiler at name lookup rather than by any value.*
   The two things a general mechanism would need -- a way to name "the `+` for type `T`," and a
   way to pass two operands into one function -- already exist separately: toylang functions are
   unary, and multi-argument calls already go through a record argument (`area {w: 3, h: 4}`,
   `docs/reference/syntax/functions.md`). A convention that looks up a fixed name derived from the
   operator and the type, and checks it as an ordinary unary function over a two-field record,
   would buy the same ad hoc polymorphism Haskell's dictionary-passing buys, except the
   "dictionary" is a name the checker resolves at compile time instead of a value passed at
   runtime -- no vtable, no `dyn`, no new value category. This is the smallest option that is
   still user-extensible, and the only one of the three that needs no new syntax.
3. *A declared interface, Rust/Swift-shaped.* A named trait or protocol, `impl for Type`, possibly
   default methods. The only option that needs a genuinely new checker concept (an interface as
   its own thing, distinct from a function signature or a type alias) and the one that most
   naturally wants a type parameter on the trait itself eventually, which is exactly the
   dependency `draft.md:2456` deferred. Worth grilling last: adopting this shape commits toylang
   to solving generics on the way, whether or not that is named as part of the ask.

Whichever of the three the session picks is a separate question from what `plans/language-
oddities.md:243-248` already settled: `concat`'s unary arity is an independent defect from Q2,
fixable regardless of which mechanism (or none) ends up carrying `+` on Vec.
