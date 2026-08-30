---
status: approved
issue: gh:105
---

# What could move to the prelude

`Opt<T>` and `Result<T, E>` already made this move (plans/opt-as-enum.md): a `Type::Opt` special
case in the checker and a bespoke encoding in every backend became a `prelude.toy` enum
declaration, checked once through the ordinary generic-enum path instead of seven times. `unlines`
and `join` made a smaller version of the same move earlier -- `src/prelude.rs:8`'s comment says it
plainly: written as toylang source, "it is checked and compiled exactly the way a program's own
functions are, so getting it right once in the checker is getting it right everywhere." This
inventory asks the same question of everything left: `tir::Builtin`, the special-cased type
formers in `src/ty.rs`, and the surface forms `src/check/mod.rs`'s `call` handles directly rather
than through the ordinary user-function path.

The short answer: almost nothing else is a clean repeat of the Opt move, and the two places that
come closest are blocked on features that do not exist yet, not on anything specific to this
survey. That is worth saying up front because "nothing to do" is a real finding here, not a
failure to find candidates.

## `tir::Builtin`, one item at a time

Twelve variants (`src/tir.rs:182-236`), every one implemented identically across all seven
backends (Go, JS, Python, Lua, jq, Rust, native/LLVM -- confirmed by grep, no partial coverage
anywhere in this enum). For each: what would have to exist for it to become a `pub fn` instead.

**`str` (`IntToStr`), `i64` (`IntToI64`).** Representation-level casts: rendering an int as its
decimal spelling, and reinterpreting a 32-bit int as 64-bit. Neither has a toylang-level
decomposition -- there is no digit-extraction-into-`Str` primitive to build `str` from (chars is
one-way, per plans/string-type-spike.md's "no `from_chars`"; `Char` has no literal to construct
digit characters with), and `i64` is a bit-width reinterpretation with nothing beneath it to
express. These stay builtins; they are not the kind of thing prelude expressiveness was ever going
to reach.

**`range`, `tail`, `concat`, `extent`.** These four are the actual Vec primitives everything else,
including the prelude's own `unlines`/`join`, is built from; that conclusion survives a
streams-first pass, but the reasons now differ per item, and one of the four turns out to have a
harder blocker than "hasn't been tried" (kantord/toylang#121: the maintainer's review of this
plan asked that every Vec-shaped candidate be re-examined for whether it should default to
`Stream`, falling back to `Vec` only for a concrete reason).

The design already has a stated position on this, not just a per-builtin one: `Stream` is second-
class and its lifecycle is fixed by
[ADR 0001](../docs/adr/0001-stream-is-the-effect-layer-typed.md) -- born only at `inputs` and
`lines`, dying only at `collect` or the `jsonlines` sink, never stored in a record, a `Vec`, or
another `Stream`. draft.md's admissible-input-set section states the memory argument directly:
"reification is where allocation becomes visible in the source, so the one operator that costs
memory is the one you have to write down" (draft.md:933-934). Every one of `tail`/`concat`/
`extent`/`sort`/`reverse` already sits downstream of an explicit `collect` today -- their memory
cost is already visible at the call site, not hidden -- so "default to streams" reads here as
"don't materialize before `collect` without a reason," which is what the per-item findings below
check for, rather than "prefer Stream wherever a retype is technically possible."

- **`range`** produces `Vec<Int>` (`src/check/mod.rs:424-430`). Making it a Stream source is not a
  mechanical retype: it would add a third stream-birth point beyond `inputs`/`lines`, which
  reopens [Q13](questions.md#q13-does-the-layer-shift-run-only-one-way-with-no-value-to-effect-operator)
  ("no value-to-effect operator is needed, because degrading a `Vec` forgets its extent and buys
  nothing") and, with it, [Q1](questions.md#q1-streams-first-class-values-or-evaluation-level-multiplicity),
  which ADR 0001 already settled the other way -- first-class, non-source-born stream values were
  considered and rejected ("a held value of genuinely unknown extent is exactly what Q13's lean
  rules out, and it is the one irreversible option"). `range` is the one item in this bullet where
  streams-first changes the finding: it is not "the floor, nothing to build it from" the way
  `tail`/`concat`/`extent` are, it is blocked on a named, already-settled design decision that a
  prelude-inventory survey has no standing to reopen on its own.
- **`tail`** (`Opt<Vec<T>>`, `tail_call` at `src/check/mod.rs:1768-1783`) is conceptually
  streamable -- peek one element, yield the rest is ordinary pull semantics -- but a
  Stream-returning version would have to return `Opt<Stream<T>>`, and `Opt` is the prelude's own
  enum (plans/opt-as-enum.md). `Type::contains_stream`'s enum arm is explicit that this is refused,
  not merely undecided: "An enum payload cannot hold a stream: resolve_enum refuses the
  declaration, and instantiation refuses a stream as a type argument" (`src/ty.rs:114-115`). That
  is a concrete, checked reason, on the same footing as `extent`'s below, not a gap this survey
  leaves open.
- **`concat`** (`Vec<Vec<T>> -> Vec<T>`, `concat_call` at `src/check/mod.rs:1788-1804`) has the
  same shape of blocker: a stream-shaped `concat` needs its argument to carry multiple streams,
  either `Vec<Stream<T>>` or `Stream<Stream<T>>`, and both are the containment ADR 0001 names
  directly ("never stored in a record, a `Vec`, or another `Stream`") -- `Type::contains_stream`'s
  `Vec` arm recurses into the element type for exactly this check (`src/ty.rs:112`). `concat`
  cannot be restated in stream form without relaxing that containment ban first, which is a
  separate, larger design question than this survey's scope.
- **`extent`** is not an open question at all here: ADR 0001's Consequences section already says,
  verbatim, "`extent` stays `Vec`-only, keeping its no-fold promise; stream reducers are future
  work." Its own doc comment gives the reason ("a dense Vec already tracks this at runtime, so
  reading it out costs nothing -- there is no fold or scan hiding behind the name",
  `src/tir.rs:203-205`): a Stream has no length until exhausted, so a Stream-typed `extent` would
  have to consume its argument to answer, trading the O(1) read for the one thing `extent`
  promises not to do. Recomputing it recursively in the prelude, over either representation, would
  still trade that O(1) read for an O(n) walk to save nothing -- unchanged from the earlier
  finding, now with the Vec-only half of it independently confirmed by the ADR rather than only
  inferred from the doc comment.

None of these four decompose into each other under either reading: `concat` is still the
language's only Vec-append (why `reverse` below builds on it rather than the other way around),
and `range` generating a Vec by counting still needs that same append, recursively, to build up --
building `range` from `concat` only relocates the problem. They stay the floor. What changed is
that three of the four (`tail`, `concat`, `extent`) have that status for a stream-containment or
no-fold reason that is now cited rather than assumed, and the fourth (`range`) has it for a
different, heavier reason: not "nothing to build it from" but "becoming a stream source is a
design decision this survey cannot make unilaterally."

**`collect`, `jsonlines`.** Not value-layer operations at all. `collect` is the one place a
`Stream` (effect layer, ADR 0001) becomes a `Vec` (value layer) -- it's wired into
`tir::fusion`/`Fusion`, which decides whether a program compiles to a fused read-one/transform-
one/write-one loop or an eager materialization. `jsonlines` is the program's only sink; there is
no expression-level "print" in the grammar, so it isn't a function reachable from ordinary
toylang at all, prelude or otherwise -- `call`'s own comment calls it "a sink, not a function."
Both are load-bearing compiler machinery, not stdlib surface.

**`fields`.** Returns a record's field names as a literal `Vec<Str>`, read off the record's
*checked type*, not any runtime value (`fields_call`'s comment: "needs no runtime support once
checked: every backend can bake the names in as a literal"). There is no reflection facility in
the language for a toylang function to ask a value what its own type's field names are -- this is
categorically different from Opt/Result, which were runtime *values* wearing a checker special
case. `fields` is a checker special case wearing a function-call spelling; nothing to migrate it
to.

**`chars`.** `Str -> Vec<Char>` decoding. Every backend does this differently because their host
strings are different representations (UTF-8 bytes, UTF-16 units, codepoint arrays -- the table in
plans/string-type-spike.md), so decoding is inherently backend-specific work, and there's no
`from_chars` encoder to build a toylang-level round-trip from anyway. Stays a builtin.

**`sort`, `reverse`.** The two "blocking" Vec-to-Vec operators (draft.md's Q20 discussion:
"`sort` is the clear case. Its cardinality is one-to-one and it is not an elementwise kernel.").
Recently landed uniformly across all seven backends by issue #86. They split apart under this
survey:

- `reverse` needs no comparison, and it turns out to be expressible with what already exists,
  with no recursion at all: `range(extent(v)) | map(v[extent(v) - 1 - .]!)` is a `map` over a
  reversed-index `range`, `.` reading the current index and `v` reaching the outer parameter the
  same way `map(a * .)` already does in docs/examples/euler/04-largest-palindrome-product.md --
  built entirely from primitives above it in this same list. That sidesteps the one real cost the
  euler-ergonomics survey found in the existing
  prelude: a recursive definition inherits "the very stack ceiling these pages engineer around"
  (plans/euler-ergonomics.md, on why a prelude `sum` isn't honest yet), because `map` compiles to
  a loop on every backend rather than to user-level recursion. `reverse` has exactly one blocker,
  and it is real: prelude functions today are monomorphic (`unlines`/`join` are declared over
  `Vec<Str>`, not a type parameter), and `reverse` needs to work over `Vec<T>` for any `T`.
  Generic *functions* don't exist -- only generic *enums* do (`enum Opt<T>`, `src/ast.rs:147`;
  nothing analogous for `fn`, confirmed against `src/parse.rs`). Moving `reverse` to the prelude is
  otherwise a one-line change, gated entirely on that feature landing first. Streams-first doesn't
  change this conclusion, only sharpens why: `reverse`'s doc comment already gives the reason,
  "blocking for the same reason `sort` is (the whole Vec has to be there before the first output
  element is)" (`src/tir.rs:232-234`) -- the same one-value-in-one-value-out cardinality Q20
  describes, which has nothing to do with which collection type holds the input.
- `sort` is blocked harder, and not only on generic functions. It needs a per-element comparator,
  and today's checker doesn't express "any orderable type" as a real bound -- `orderable()`
  (`src/check/mod.rs:1810`) is a hardcoded allowlist (`Int`, `Int64`, `Str`, `Char`), not a trait a
  generic function could be written against. Even with generic functions, `sort` would need that
  bound to exist as a real language feature, not a checker-internal list. And separately, draft.md
  takes a design position that blocking whole-collection operators like sort should eventually be
  built from the parallel primitive basis (scan/gather/scatter) rather than from recursion --
  exactly the shape a hand-written insertion or merge sort in `prelude.toy` would be. Three
  stacked blockers (generic functions, a real orderable bound, and a standing design preference
  against a recursive definition for this specific operator) make `sort` a poor candidate to
  revisit until at least the first two exist independently of this survey.

## Checker-special-cased type formers (`src/ty.rs`)

`Type` has ten variants; two of them, `Opt` and `Result`, already left this list and are why this
survey exists at all (`takes_type_arg`'s comment: "`Opt` is no longer here: it is the
prelude's enum"). What's left -- `Str`, `Int`, `Int64`, `Bool`, `Char`, `Vec`, `Stream`, `Record`,
`Enum`, `Param` -- has no member shaped like Opt/Result was. `Vec` and `Stream` are type
*formers* with dedicated grammar (`<...>` in type position is special-cased to exactly these two
names, `takes_type_arg`, `src/ty.rs:90-92`) and their own runtime representation on every backend
(dense buffer vs. effect-layer iterator, ADR 0001) -- there is no enum shape underneath either of
them to declare in `prelude.toy` the way `some(T)`/`none` underlies `Opt`. The five scalars are
primitives with no internal structure to speak of. None of these are checker special cases in the
sense Opt was (a value wearing a bespoke per-backend encoding); they're the type grammar itself.
Nothing here is a candidate.

## Surface forms special-cased in `check::call` (`src/check/mod.rs:1574-1865`)

`select` and `map` are excluded on inspection: they rebind `.` inside their argument, which no
user-defined function can do (`call`'s own comment: "not special syntax, only special names...
which no ordinary function needs and is why they cannot be defined as one"). That's a binder, a
different kind of thing from a prelude function entirely, not a migration candidate.

Everything else special-cased here (`extent`, `tail`, `concat`, `fields`, `sort`, `reverse`,
`jsonlines`) is special-cased *because* it lowers to a `tir::Builtin` or a sink, which the section
above already covers per-item. There's no separate checker-only special case in this file beyond
what the `Builtin` enum already accounts for -- `check::call` is the front door, not an additional
layer of special-casing on top.

## Backend-emitted runtime helpers

Every backend synthesizes a handful of named helpers into its output, gated by a per-backend
"used" tracking struct (e.g. `emit_go.rs`'s harvesting walk at `src/emit_go.rs:513-645`, `emit_js.rs`'s
`used.tail`/`used.chars`/`used.jsonlines` flags, `emit_lua.rs`'s `builtin_helpers`). Representative
examples: Go's `tlTail`/`tlConcat`/`tlSort`/`tlReverse`, Rust's `tl_tail`/`tl_concat`/`tl_sort`,
Python's `tl_range`/`tl_tail`/`tl_vec_concat`, the native runtime's `tl_vec_tail` and friends in
`runtime/toylang.c`. Every one of these is the emission side of a `tir::Builtin` variant already
covered above, not a fourth category: they exist because a `Builtin` variant needs runtime support
on that particular target, and the question "could this be one prelude function instead of seven
backend-specific ones" was already asked and answered per-variant. There's no helper here that
isn't accounted for by the `Builtin` inventory -- printers (`tl_show_*`, `src/ty.rs:267-269`) and
the recursive-enum printer functions (kantord/toylang#94) are a separate, structural concern (each
backend's own value representation) rather than stdlib surface, and are out of scope for the same
reason `Vec`/`Stream` themselves are.

## What this leaves as actual next steps

Nothing here is shovel-ready work the way `opt-as-enum` was, because the one plausible near-term
candidate (`reverse`) is gated on a feature (generic functions over `fn`, not just `enum`) that
doesn't exist and is a real design undertaking of its own -- not a follow-up row this plan can
spin off directly. If generic functions land for other reasons (matcher-totality-and-alt-design
and the combinator-library duplication noted in plans/matcher-parser-spike.md are both already
pulling in that direction), `reverse`-to-prelude becomes a same-day follow-up worth a row at that
point. `sort` should wait on the same feature plus a real orderable bound, and probably longer
given draft.md's standing preference for a parallel-basis implementation over a recursive one.
Everything else in this inventory (`str`, `i64`, `tail`, `concat`, `extent`, `collect`,
`jsonlines`, `fields`, `chars`) has a specific, load-bearing reason to stay exactly where it is,
not merely "hasn't been tried yet." `range` is the one item whose reason is a different kind of
thing: not a closed design fact like `extent`'s no-fold promise, but an open question (Q1/Q13)
this survey found and does not have standing to answer. Whether `range` should become a Stream
source is a decision for the maintainer, not a follow-up row -- it would mean amending ADR 0001,
not just landing a feature it already depends on.
