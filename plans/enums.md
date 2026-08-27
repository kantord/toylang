# Enums: the first slice

Implements the design in draft.md's "DECIDED: enums, nominal and JSON-native" and ADR 0009.
Everything semantic is already decided there; this plan only orders the build and names where
the effort actually is. Prioritized ahead of the stream type-system work, per that decision.

The one sentence that shapes the whole plan: **an enum value is plain JSON** -- a bare string
for a unit variant, a single-key record for a payload variant. Six of the seven backends can
therefore construct and print enum values with machinery they already have. Nearly all of the
genuinely new work is in the checker (nominal types, exhaustiveness) and in the native
backend's memory layout, and the steps are cut so that the cheap majority lands before the
expensive minority.

## Step 1: declarations, constructors, printing

`enum Shape { point, circle{r: Int} }` parses in a program and in `prelude.toy`'s module form.
The checker gets a nominal `Type::Enum` keyed by name, an enum registry, and the constructor
surface: `circle{r: 1}` resolves like an application (the Q34 constructor path), a bare
`active` resolves through the registry while exactly one enum in scope declares it, and
`Shape.circle` is the qualified spelling (uppercase-then-dot is unambiguous under the casing
rule, so this is a small parser case, not new syntax design).

A constructed enum value lowers to the string or single-field record every backend already
builds, and prints as JSON -- including a top-level unit variant, which prints `"active"` with
quotes, because the raw-output rule keys on the type being exactly `Str` and `Status` is not
`Str`. A corpus case pins that.

Rejection tests (step_N.rs, insta): duplicate variant within an enum; two enums declaring the
same variant name plus a bare use of it (the error must name both candidates); `Shape.circle`
where `Shape` has no such variant; a payload where a unit was declared and the reverse.

Deliberately absent in this step: match. Construct-and-print is already a shippable,
corpus-testable slice.

## Step 2: match, exhaustive

The surface form comes from the pattern-matching sketch (draft.md, "Pattern matching is
decoding"): arms are `pattern -> body` chained with `//`, first match wins, `.` rebinds to the
payload inside an arm, a bare name in a record pattern binds fresh (`circle{r} -> r * 2`).
This step implements only the closed-world branch that section reserved: the subject's type is
a declared enum, so the checker proves the `//` chain covers every variant or ends in a
default arm, and no `Result` exists anywhere. The `Matcher` algebra, `and`/`or`/`not`, and
dynamic `Json` decoding are all out of scope.

What has to be decided at implementation time, within the sketch's constraints rather than
freshly: the exact spelling of the subject position (the sketch never fixed whether a match is
`subject | arms` or a keyword form). Whichever is chosen, the arms and `//` semantics are
fixed already.

The checker gains the exhaustiveness proof and payload-type narrowing per arm. TIR gains a
match node; `tags.rs` gains its node types so the corpus tree can browse enum cases.

Rejection tests: a non-exhaustive match naming the missing variants; an arm for a variant the
enum does not have; an unreachable arm after a default. The corpus cannot see exhaustiveness
at all -- it is a compile-time property, the same blindness the streaming work hit -- so these
step tests are the only witness it has.

## Step 3: the six easy backends

Lua, JavaScript, jq, Go, Python, Rust: construction is a string or record literal, match
lowers to a shape test -- string equality for unit variants, single-key presence for payload
variants -- plus a binding. jq is the one to watch, as usual: its match will lean on
`has`/`type`, and the raw-output flag rule from step 1 needs checking against real jq, not
assuming. Snapshot one enum corpus case per the usual practice, and let the agreement harness
do the rest.

## Step 4: the native backend, which is the long pole

A scalar `Shape` value is easy. `Vec<Shape>` is not: the native backend stores `Vec<Record>`
as struct-of-arrays, and an enum is precisely a value whose shape varies per element, which
columnar layout cannot represent directly. This is the dense-union problem (Arrow solves it
with a tag buffer plus one child buffer per variant), and it is an open implementation choice,
not decided by the design: tag-plus-per-variant-columns (columnar, vectorizable, more work) or
a boxed per-element representation for enum-typed columns (simpler, slower, a special case in
a backend built on not having special cases). Whoever takes this step should decide it looking
at `runtime/toylang.c` as it actually is, and record the choice in the research log -- the
struct-of-arrays invariant already has four independent construction sites, and this adds
enum-aware ones.

`tl_parse` also needs an enum descriptor for step 5, so the descriptor design here should
anticipate it.

## Step 5: enum-typed input

`input` and `inputs` with an enum-typed parameter: validation accepts a bare string matching a
unit variant or a single-key object matching a payload variant, and rejects everything else
with a message naming the enum. This is `expect()` work in the checker plus per-backend reader
validation, and on native it is the `tl_parse` descriptor from step 4. The payoff corpus case
is the motivating one: NDJSON of mixed messages, `inputs` typed `Vec<SomeEnum>`, one match in
a map -- the heterogeneous-stream story working end to end on all seven backends, eager today,
fused once the stream work lands.

## What this plan does not include

Generics (`Result<T, E>` belongs to the decoding work), the `Matcher` algebra, literal-as-enum
typing (`"active" : Status` -- recorded as the second forcing case for bidirectional
checking), tag-field codecs, and any stream-type integration beyond the eager step-5 case.
Each is named in the DECIDED section with its own trigger.
