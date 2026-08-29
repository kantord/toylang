# Opt becomes a prelude enum

Implements the ratification recorded on kantord/toylang#29 and filed as kantord/toylang#62:
generic enums land, `Opt<T>` stops being a built-in and becomes a prelude declaration over
them, absence is tagged in memory everywhere, and `null` survives only at the serialization
boundary. This retires draft.md's "Opt is provably not self-hostable as an enum" (the proof
assumed the canonical form is the value) and the "first cut is monomorphic" sentence, whose
reserved first customer was `Result<T, E>`; `Opt` takes the slot.

Two facts shape the whole plan.

First, three of the seven backends already store absence tagged: Rust uses a real
`Option<T>`, Go a `tlOpt[T]{ok, v}` struct, and the native runtime a boxed slot where `NULL`
is absent -- in all three, an absent outer and an absent inner are different values already.
Only the four backends that borrowed their host's null-ish value (JS's `Symbol("none")`,
Python's `None`, Lua's `tl_none` sentinel, jq's actual `null`) conflate levels and need a new
encoding. The re-encoding work is half the size the issue's phrasing suggests, and no
backend's output changes at all, because the bare-value-or-`null` serialization is exactly
what every printer already emits.

Second, the user-facing surface for consuming an `Opt` by cases is owned by a design round
that has not run: the board row `matcher-totality-and-alt-design`, queued from the three
threads the maintainer opened when closing kantord/toylang#47 (coverage measurement through
first-class matchers, alt-composition, the parser-combinator stress test). This plan builds
strictly below that surface -- the generics layer, the type-level switch, the encodings --
and where the surface would begin, it stops and says so. The parameterized points are
collected in [Open points, owned elsewhere](#open-points-owned-elsewhere).

## The spelling

```
pub enum Opt<T> { some(T), none }
```

Lowercase variants, scalar payload. Both halves need defending, since the issue sketches
`Some{x: T}, None`.

**Lowercase.** The auto-matchers ratification (draft.md, "Construction and naming", revised
2026-08-29) makes the end state: variant names are capitalized types, a capital name in
expression position is the derived matcher, and the constructors users write are lowercase
-- `some(v)` a unary function, `none` a plain constant. Today's shipped machinery derives
the constructor spelling from the declared variant name. Declaring `Some`/`None` now would
therefore expose `Opt.Some(1)` and bare `Some(...)` as constructors through the existing
qualified and bare paths -- the exact spelling the ratification reassigns to matchers -- and
that surface would have to break when the `variant-types-flip` row lands. Declaring
lowercase gives users `some(5)` and `none` today, which is verbatim the ratified end-state
constructor surface; what the flip later migrates is the declaration's own spelling and the
type/matcher namespace, not anything a user program wrote. The prelude line is one edit
then, alongside every other enum in the corpus.

**Scalar payload.** `some(T)` over `some{x: T}` because the field name buys nothing: no
caller ever projects `.x` (the consumption paths are `!` and, later, matchers), the printer
wants the payload itself, and on the native backend a record payload is a second allocation
per present value. The parens spelling for scalar payloads is already shipped
(`enum_scalar_payload` in the corpus).

## Step 1: generic enums

Everything here is useful on its own and touches no Opt machinery, so it lands green first.

The declaration grows type parameters: `enum Pair<T> { two{a: T, b: T}, empty }`. Parameters
are capitalized names (they are types), bound only inside the declaration, refused if they
collide with a declared or built-in type name. In the type grammar, `Pair<Int>` stops being
special to `Vec`/`Stream`/`Opt`: any named type may be followed by `<...>` in type position
(no ambiguity exists there), and `takes_type_arg` retires in favor of an arity check at
resolution -- `Pair` alone and `Str<Int>` both become errors that name the arity.

Resolution instantiates by substitution. `resolve_enum` gains a parameter binding; type
arguments resolve first (so the `seen` recursion chain stays about the enum's own name and a
self-referential payload is still caught), then the payloads resolve with parameters bound.
The instantiated `Type::Enum` carries its `args` alongside `name` and the substituted
`variants`: identity is name plus args, display is `Pair<Int>` -- which is also why every
existing error message that prints `Opt<Int>` survives step 2 verbatim. A stream as a type
argument is refused at instantiation, message naming the enum, for the same reason the
payload ban exists.

Constructors need inference. Payload variants infer bindings by unifying the declared
payload type against the synthesized payload (`two{a: 1, b: 2}` binds `T = Int`); a
parameter the payload leaves unbound is refused. A unit variant of a generic enum cannot
synthesize its arguments at all -- `empty` alone is the `[]` problem again -- so it is
refused in synthesis position ("cannot tell what `Pair<...>` this is") and constructed
through the expectation in check mode, which the type-flow rework already delivers
(`fn f() -> Pair<Int> = empty`).

Backends: the six easy ones represent an enum value the same way at every instantiation, so
they need nothing. Go and Rust name a struct/enum per declared enum (`tlE_Pair`,
`TlE_Pair`), which collides across instantiations, so the emitted type name gains a mangled
suffix derived from the args (`tlE_Pair_Int`), and their type-harvesting keys on the
instantiated identity. Enum-typed input should fall out of the instantiated variants riding
the type; verify with a test rather than assuming, and if the native descriptor path fights,
refuse generics in input types for this step and record it.

Tests: refusals for arity, unknown parameter, parameter/type-name collision, stream
argument, and the unbound unit variant (step-test snapshots, since none of these reach a
backend); corpus cases for construct-print and match over a generic enum on all seven
backends (match needs no new machinery -- arms read the substituted variants off the
subject's type); a module-form parse test for `pub enum Opt<T>` ahead of step 2.

## Step 2: Opt moves into the prelude

The switchover. `Type::Opt` is deleted; `prelude.toy` declares the enum; the four producer
sites in the checker (collapsing index, `tail`, the partial-chain wrap, and the written
annotation, which now resolves through step 1's generic path) build the instantiated
`Opt<T>` enum type instead. A helper answering "is this type the prelude `Opt`, and of
what?" replaces every `Type::Opt` pattern-match -- the unwrap operator's check, the
`!=`-reads-as-one-token diagnostic, the printers' type dispatch. `Opt` leaves the reserved
names; redeclaring it now collides with the prelude's declaration and gets the ordinary
"type defined twice" error (the pinned "built-in and cannot be redefined" snapshot updates).

What this makes true, and the tests that pin it: `Opt<Opt<T>>` is two distinguishable levels
in memory. Constructors `some(v)` and `none` exist because the enum exists -- nothing grants
them specially -- and `some(5)` prints `5`, `none` (through a return annotation) prints
`null`. No existing corpus output changes anywhere.

**Serialization.** A present value prints as its payload, an absent one as `null`,
recursively -- so serialization flattens every level of tagging and `some(none)` also prints
`null`. Lossy by design, the way serialization already drops every other type-level
distinction; that is the ratified trade. This is the one place `Opt` stays special: the
printers key on the Opt helper and emit payload-or-null instead of the JSON-native enum
form. It is the first type-directed serialization override in the tree; the general codec
layer stays future work.

**Encodings.** Per backend:

- Rust keeps `Option<T>` (already tagged); `rs_type` maps the Opt enum to it, and the
  `TlE_` harvest excludes Opt. Emitted code is unchanged.
- Go keeps `tlOpt[T]{ok, v}` (already tagged), same treatment.
- Native keeps `NULL`-or-boxed (already tagged: `some(none)` is a box holding `NULL`, which
  is not `NULL`). `tl_at`, `tl_vec_tail`, `tl_opt_*` in `runtime/toylang.c` do not change.
  This leaves Opt's native layout different from the general two-slot enum box; that is
  legal while nothing matches on an Opt (see the open points), it sits below the emit
  boundary where the corpus cannot see it, and the reconciliation -- if the matcher round
  ever forces one -- is the reversible kind, per the argument recorded in plans/enums.md
  step 4. Goes in the research log either way.
- JS, Python, Lua, jq re-encode to the enum's own JSON-native shape: `{"some": v}` present,
  `"none"` absent (object/table/dict per host). This is what makes them tagged, and it is
  also the shape their existing enum match machinery expects, so the day the matcher
  surface lands, these four get it for free. Their sentinels (`Symbol("none")`, `None`,
  `tl_none`, raw `null`) are deleted; the absence tests in `tl_at`/`tl_tail`/unwrap
  helpers/partial-chain fall-throughs become tag tests; the printers map the tagged shape
  to payload-or-`null`. jq keeps one nicety: an in-memory element can never be JSON `null`
  once the sentinel is gone, so `.[i] == null` still means exactly out-of-range inside its
  `tl_at`.

**Refusals kept or added, on purpose:**

- Match with an Opt subject is refused: how `some`/`none` arms compose and what totality
  they owe is precisely what the pending round decides, so shipping the closed-world
  exhaustive match for Opt now would guess. The message says the thing is not decided
  rather than pretending the type is unmatchable.
- Opt anywhere in an `input`/`inputs` type is refused with a real checker error (today it
  is an `unreachable!` two backends deep, with a stale message). Absence on the wire --
  whether input `null` should read as `none` -- is codec design nobody has done, and the
  uniform enum wire form (`{"some": 5}`) would make input and output asymmetric. Refusing
  is the reversible direction.
- The two-nulls refusal on partial chains survives this step unchanged (re-keyed on the
  helper), so step 2's diff stays about representation, not semantics.

The stream-containment refusals move: `Opt<Stream<Int>>` now dies at generic instantiation
(step 1's rule) rather than at a dedicated `TypeExpr::Opt` arm, so the three pinned
"an Opt cannot hold a stream" snapshots update to the instantiation message.

Docs touched here only where they now lie: the Opt reference's built-in framing, the unwrap
page's "Opt cannot be declared" sentence, and an amendment note on ADR 0009's
"provably not self-hostable" clause pointing at the ratification. The full rewrite of the
Opt reference (the why-absence-had-no-representation unpacking, the Rust comparison the
maintainer asked for) is the `opt-docs-unpacking` board row, deliberately not this plan.

## Step 3: the two-nulls relaxation

With tags real, the refusal at the partial-chain wrap ("a declined chain and a
matched-but-absent value would both print `null`; add a default arm") loses its premise in
memory: the chain that declines yields `none`, the arm that matched-and-found-nothing yields
`some(none)`, and a program can tell them apart -- today through `!`, which peels exactly
one level, later through matchers. At the serialization boundary both still print `null`,
and after step 2 that is a documented property of serialization, not a conflation the
checker must prevent. The ratification says the refusals "relax wherever the tag genuinely
distinguishes"; this is that site (the only one -- the survey found no second).

So the refusal is removed, and a partial chain over Opt-bodied arms types as `Opt<Opt<T>>`.
The program draft.md said this rule existed to refuse -- `map(.valid -> .readings[0])`
printing `[null, null]` -- becomes a corpus case pinning the ratified lossiness instead. Two
more corpus cases pin that the tag is real where serialization cannot show it: the
matched-but-absent value unwrapped once prints `null`, the declined chain unwrapped once
refuses at runtime.

The docs that teach the refusal (docs/guides/matching.md's partial-chain section with the
verbatim error, docs/tutorial/06-matching.md) are rewritten to teach the distinction
instead, which keeps the docs harness green.

This step changes the standing of kantord/toylang#48 (should a partial chain's arms receive
a peeled Opt expectation?): the refusal whose explanatory message option 2 there existed to
preserve is gone, so what remains open in #48 is only whether peeling happens at all -- the
conservative no-peeling cut stays untouched by this plan, and #48 gets a comment saying its
option space shrank.

## Open points, owned elsewhere

Parameterized, not decided here. Each names its owner.

- **Totality of `some`/`none` arms** -- whether a match over Opt must be closed-world
  exhaustive like any enum, or flows through first-class matchers with measured coverage.
  Owner: the `matcher-totality-and-alt-design` round (thread 1 of kantord/toylang#47's
  closing comment). Until then: match-on-Opt refused (step 2), `!` and the producers'
  combinators are the consumption surface.
- **Alt-composition** -- one arm serving several left-hand sides. Owner: the same round
  (thread 2). No Opt-specific work exists to do ahead of it.
- **Variant casing** -- the declaration's `some`/`none` become `Some`/`None` as types and
  matchers when `variant-types-flip` lands (blocked on the same round). The prelude
  declaration is one line in that migration; the constructor surface this plan ships is
  already the flip's end state.
- **Opt expectation peeling into partial arms** -- kantord/toylang#48, reduced but not
  closed by step 3.
- **Absence on the wire** -- whether input `null` reads as `none`. No owner yet; the step 2
  refusal is the placeholder. Becomes a decide row only if a program actually wants it.

## What this plan does not include

Generic type aliases (nothing needs one; `parse_module` still refuses `type`, and that
stays), the `Matcher` type and everything downstream of the pending round, tag-field
codecs, a native dense-union or any layout unification, and auto-wrapping (`5` where
`Opt<Int>` is expected stays refused; presence is written `some(5)`).
