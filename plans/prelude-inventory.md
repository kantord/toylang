---
status: needs-changes
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
including the prelude's own `unlines`/`join`, is built from. `tail` and `extent` are the base case
of every recursive prelude function that exists today (`prelude.toy`'s `unlines`: `"" if
extent(v) == 0 else ... tail(v)!`) -- `extent`'s doc comment is explicit that it is meant to cost
nothing ("a dense Vec already tracks this at runtime... there is no fold or scan hiding behind the
name"), so recomputing it recursively would trade an O(1) read for an O(n) walk to save nothing.
`concat` is the language's only Vec-append: nothing else turns two `Vec<T>`s into one, which is
also why it's the thing `reverse` below would build on rather than something reverse could help
retire. `range` generates a Vec by counting, which needs the same append primitive `concat` is,
recursively, to build up -- so building `range` from `concat` only relocates the problem, it
doesn't remove a builtin. None of these four decompose into each other; they're the floor, not
candidates standing on it.

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
  otherwise a one-line change, gated entirely on that feature landing first.
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
Everything else in this inventory (`str`, `i64`, `range`, `tail`, `concat`, `extent`, `collect`,
`jsonlines`, `fields`, `chars`) has a specific, load-bearing reason to stay exactly where it is,
not merely "hasn't been tried yet."
