# Surveying emuto, the predecessor

Issue #84. emuto (github.com/kantord/emuto) is toylang's stated predecessor, and this project may
eventually fold back into that repository, so the brief is to read the actual code, not sixteen
years of memory: `git clone https://github.com/kantord/emuto.git`, 518 commits from
2018-08-09 to 2024-12-19, MIT-licensed, 222 stars, 8 forks, 49 open issues,
**archived** (`gh repo view` reports `isArchived: true`). The last commit is a dependency-bot
YAML edit; the design work stopped years before the archival.

## What emuto is

A small language for "manipulating and restructuring JSON and other data files," explicitly
positioned against jq and GraphQL (`docs/comparison_with_other_languages.md`). It ships as three
things from one codebase: an npm library (`emuto`), a separate CLI package (`emuto-cli`, not in
this repo), and a webpack loader, and the README's selling point is that all three run in a
browser as well as node -- a constraint that shows up throughout the implementation.

The pipeline is parser combinators to an AST to a JavaScript source string to `eval`
(`src/compiler.js`: `` `(function(_) { return (function(input) { return ${Generator(...)} })})` ``,
consumed by `src/interpreter.js`'s `eval(compiler(sourceCode))`). There is one backend. Flow
(`// @flow`) types the *implementation*, not the language: an emuto program is dynamically typed
JSON-in, JSON-out, and nothing about a program is checked before it runs. `map`, for instance,
dispatches on the runtime shape of its input (`Array.isArray(input) ? input.map(f) :
objectify(Object.entries(input).map(...))`, `src/builtins.js`), the same way jq's polymorphic
builtins do.

## The four axes named in the brief

**Pipes.** Structurally the same operator toylang has: `.foo | .bar` feeds output to input,
left to right, and `$` (emuto) / `.` (toylang) is the value in flight
(`docs/chaining_filters.md`, `docs/reference/operators/pipe.md`). Both languages borrowed this
from jq rather than from each other by way of jq being the actual common ancestor; nothing here
is a design choice specific to either project.

**Matching.** No comparison is possible because emuto has nothing that plays this role. It has a
ternary (`"Foo" if 3 < 4 else "Bar"`, `docs/basic_filters.md`) -- the same postfix
condition-and-two-branches shape toylang's own conditional uses, right down to the Python
keyword order (`docs/reference/operators/conditional.md`) -- and nothing else that branches on a
value's shape. There is no closed set of variants, no exhaustiveness, no compile-time coverage
check, because there is no compile time. toylang's match (`docs/reference/operators/match.md`) is
this project's own answer to a question emuto never had a mechanism to ask.

**Stdin.** emuto reads one thing, whole, and format is a CLI concern: the (separate, unreviewed)
`emuto-cli` package selects the input shape with a flag (`-i=raw` for line-oriented text, plus
CSV/TSV/DSV per the README's feature list) before the emuto program ever runs, and the value it
hands the program is always a single parsed document. There is no streaming story anywhere in
this repo -- nothing corresponding to `inputs` or `lines`. toylang's three typed sources
(`docs/reference/sources/{input,inputs,lines}.md`) answer a question emuto's architecture could
not have raised: the CLI-side format flag picks *parsing*, never *multiplicity*, because
multiplicity would need a value to be validated against a shape the interpreter does not track.

**Types.** This is the real axis, and the other three are downstream of it. emuto has none:
`convertUndefined` (`src/builtins.js:15`) turns every `undefined` into `null` at every property
access, silently, so a missing field and a present-but-null field are frequently
indistinguishable by the time a program sees them -- exactly the ambiguity
[`Opt`](../docs/reference/types/opt.md) exists to make impossible (`null` only ever means an
absent `Opt`, and an empty `Vec` still prints `[]`). Optional chaining (`?.`, `?[`) is emuto's
entire error-handling story: walk through absence and keep going. toylang's answer is to make
absence a value with a type (`Opt<T>`) that must be named in a signature and consciously
discharged (`!`), and to validate stdin against the declared type before the program runs at all
(`docs/reference/sources/input.md`). Where emuto has one dynamic value type that degrades
gracefully, toylang has records, enums, `Opt`, and a checker that runs first.

## What emuto got right that toylang currently lacks

Two are plain builtin gaps, not design questions. `sortBy` (`docs/functions.md`) sorts a Vec by a
key function; toylang's builtin set (`docs/reference/builtins/*.md`) has no `sort` or `sort_by`.
draft.md reaches for `sort_by(-.value)` in one of its two earliest worked-program sketches
(draft.md:1291) -- illustrative, pre-dating the settled syntax, and not itself evidence of a
decision -- but Q20 does settle the actual design question, classifying `sort` as a blocking,
one-value-in-one-value-out operator with no lawful stream instance (draft.md:2752-2759). The
classification is decided; the Vec-level builtin implementing it is not written. `reverse`
(`docs/functions.md`) has no toylang equivalent either. Both are common enough in real jq-style
scripts that their absence is felt in the CLI-replacement beachhead specifically, not in some
speculative future feature.

The third is a real gap with no existing toylang answer even in draft form: emuto's trailing
`where` clause (`docs/variables.md`) names one or more values for reuse inside a single
expression --

```
$one + $two

    where $one = 1
          $two = 2
```

-- and toylang has nothing that plays this role. The only way to name a subexpression today is
to factor it into a top-level `fn`. draft.md already reaches for the same shape twice without
committing it: `("red","blue") as $c | db.color = $c` in the still-open mutation section
(draft.md:1263), and draft.md:628 names the gap outright -- "'Aliasing' a submatch so later code
can refer to it by name is also already in the document, just not yet generalized" -- while
sketching `as` binding a matched submatch as one further step past binding a whole expression's
result. Nothing about the record/`Opt`/match design forecloses adding it; it is an orthogonal,
unaddressed hole that emuto's own examples (the `restructure.emu` script) lean on constantly.

## What it abandoned, and why that reads as vindication rather than as a lost feature

emuto's single load-bearing design choice -- compile straight to a JavaScript source string and
`eval` it -- is also its worst one, and it explains most of the rest. Dynamic typing was table
stakes for shipping a JS-hosted interpreter in 2018; it correspondingly means an emuto program's
first correctness check is the person reading its output. There was never a second backend, so
there was never a cross-implementation check on the language's own semantics the way toylang's
seven backends serve as falsifiers of each other (draft.md's "What this is"): a bug in the one
JS codegen path is indistinguishable from the language's actual behavior, because there is
nothing else to disagree with it. `eval`-ing generated source is also a straightforwardly
different security posture than toylang's compiled backends, worth naming even though nothing in
this survey suggests toylang would ever adopt it.

The GraphQL-shaped object projection -- `$ { firstName, age, home { city } }`, with `...Fragment`
spreading a named function's result into the object (`docs/object_projections.md`) -- is emuto's
most distinctive syntax and its headline comparison point against GraphQL. It is also precisely
the sugar draft.md already considered and declined, under the name "punning":

> `{name}` for `{name: .name}` is jq's most-used shorthand and is not being adopted, for a reason
> better than conservatism: it would answer a question by abbreviation. Narrowing a record to
> some of its fields is arguably its own operation, the way `select` narrows a dimension, and the
> glossary has no term for it because the language has not decided.
> (draft.md:1818-1824)

emuto's fragments are, underneath the projection syntax, ordinary functions from record to
record (`$Address = ($ => $ { city, state })`) -- a shape toylang already has, spelled as an
ordinary `fn` applied to a projection (`address(.home)`). The one piece of emuto's design that is
not already reachable this way is the field-picking shorthand itself, and that piece is the exact
thing already declined by name.

Functions-as-values are the other settled non-import: emuto's `sortBy`, curried lambdas
(`$key => $value => ...`), and fragment spreading all lean on lambdas being ordinary JS closures
that can be stored and passed. toylang's functions cannot be stored, passed, or returned
(`docs/reference/syntax/functions.md`), a constraint chosen deliberately rather than one that
merely hasn't been lifted yet, and every emuto builtin that reads as "takes a function" (`map`,
`filter`, `sortBy`, `reduce`) already has a toylang answer that takes an *expression* over `.`
instead (`docs/reference/builtins/{map,select}.md`). Nothing here is missing; it was already
decided the other way, for reasons unrelated to emuto.

`has` (key-existence check) and `keys`/`values`/`entries` (`docs/functions.md`) are artifacts of
records having no fixed shape at emuto's runtime. toylang's `fields` builtin
(`docs/reference/builtins/fields.md`) already gives the field-name half of this statically, from
the type rather than from a value inspected at runtime, and `has` has no meaning once field
presence is a checked fact rather than something to probe for. `combinations` and `product`
(`docs/functions.md`) are niche math helpers with no connection to either language's actual
design questions, and are not worth weighing either way.

## Recommendations

- **adopt** -- `sort_by` (or `sort`) as a Vec builtin. Its classification is already settled
  (Q20, draft.md:2752) and it is a common-enough jq idiom that its absence is a real gap in the
  CLI beachhead today; only the implementation is missing.
- **adopt** -- `reverse` as a Vec builtin, for the same reason: no open design question blocks
  it, and it is missing purely because nobody has written it yet.
- **adopt, as a design question to open rather than a spelling to copy** -- some form of local
  binding for naming a subexpression inside one expression, in the shape of emuto's trailing
  `where` or jq's `as`. draft.md already names this as an ungeneralized gap (draft.md:628,
  draft.md:1263); emuto's `where` is one existence proof of a shape that works, not a
  recommendation to copy its exact syntax.
- **already-superseded** -- the GraphQL-style object-projection shorthand (`{ firstName, age }`
  as sugar for picking fields). draft.md already declined this by name as "punning"
  (draft.md:1818), for a reason -- it answers the undecided "narrowing a record" question by
  abbreviation -- that applies just as much to emuto's version as to jq's.
- **already-superseded** -- optional chaining (`?.`, `?[`) and silent undefined-to-null
  conversion as the error-handling model. `Opt<T>` plus `!` is a strictly more precise answer to
  the same problem (absence is typed and must be named in a signature, rather than propagating
  silently through every access), and toylang's static input validation
  (`docs/reference/sources/input.md`) removes most of the cases optional chaining exists to
  survive in the first place.
- **already-superseded** -- dynamic typing and runtime-shape-dispatching builtins (`map` on
  Array-or-Object). toylang's whole differentiation from emuto is the static type layer; nothing
  here is worth re-examining.
- **avoid** -- codegen-to-a-single-backend-and-`eval` as an implementation strategy. Not a live
  option toylang was ever going to take, but worth naming as the concrete cost emuto paid for
  it: no cross-backend semantic check ever existed, so a codegen bug and the language's actual
  semantics were indistinguishable for the project's entire life.
- **avoid** -- functions as storable, passable, returnable values. Already decided against
  (`docs/reference/syntax/functions.md`), for reasons independent of emuto; every emuto use case
  that leans on this (`sortBy`, curried lambdas, fragment spreading) already has a toylang answer
  that does not need it.
- **worth a look, lower priority** -- CSV/TSV/DSV as recognized stdin formats. This lives in
  `emuto-cli`, not in this repo, so it was not reviewed directly here; it is real evidence that a
  jq-replacement CLI tool's users want non-JSON structured input, which toylang's `input` /
  `inputs` / `lines` trio does not currently cover (only whole-JSON, JSON Lines, and raw text
  lines). Worth a future issue rather than action now, since it is a source-and-parsing question
  orthogonal to everything else here.

## On the possible future repo merge

There is no code to merge -- emuto is a JavaScript/Flow codebase with a hand-rolled
parser-combinator front end and a single JS-codegen-and-eval backend; toylang is Rust with seven
backends and a from-scratch front end (draft.md: "The front end is written from scratch"). A
merge, if it happens, would be a redirect (archive emuto with a pointer to toylang, or rename/
retarget the repo) rather than an integration of any source. Three things worth planning around
before that happens, none of them urgent:

- The repo carries 222 stars, 8 forks, and 49 open issues that a redirect would need to account
  for -- likely by closing the issues with a pointer to toylang rather than triaging them
  individually, since none of this survey's reading suggests emuto's open issues describe
  problems toylang has independently rediscovered.
- The `emuto` name is a published, unarchived-if-ever-revived npm package; if toylang ever wants
  that name (unlikely, given it is not called emuto and has its own identity), that is a
  separate decision from the repository merge and should not be assumed as part of it.
- MIT license on both sides removes any legal blocker to reusing text or examples verbatim from
  emuto's docs, if a future migration guide or comparison page wants to quote them directly
  rather than paraphrase, though nothing in this survey copies emuto source text for that
  reason -- everything here is described rather than quoted, per this repo's own rule.
