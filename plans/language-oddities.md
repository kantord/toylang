# Language oddities: an inventory

What issue #9 asked for: everything in the language as it exists -- draft.md, CONTEXT.md,
prelude.toy, the corpus, and the implemented surface in src/parse.rs and src/check.rs -- that
deserves a deliberate look. Duplications, needless complexity, cryptic naming, and things that
fit badly whether inherited from jq or invented here. Inventory and framing only; every item is
left for a review session to settle, and nothing here is a recommendation.

Every program fragment below was run against the compiler at the commit this file lands on
(`toylang run`, Lua backend; parse and check errors are backend-independent). Output and error
text are quoted as produced, not paraphrased.

The list is short on purpose. Things that looked odd but turned out to be decided with reasons
that still hold (truncating division, wrapping Int, the Python-shaped conditional, punning's
rejection, bare-until-ambiguous variant names) are not re-listed; the open-questions table
already guards those.

## Three stdin keywords, pairwise exclusive, two of them one letter apart

```
input     one JSON value, read whole, typed by use
inputs    Stream<T>, one JSON value per line
lines     Stream<Str>, raw lines
```

Any two of them in one program is a compile error ("they read the same real stdin two different
ways"). The exclusivity itself is measured, not chosen: Python's `input` drains stdin and jq
cannot run in raw and parsed mode at once. What is chosen is the surface: three keywords for one
resource, where `input` vs `inputs` differ by one typed letter and by an entire type discipline
(a value checked against use, versus a linear stream). The names are jq's, and jq's own `input`
/`inputs` pair is widely considered one of its most confusing corners.

`input` is also documented scaffolding: the draft says outright that it is "a different
construct standing where `stdin` will go." So one of the three is already scheduled to die,
which is an argument for deciding the reader surface once rather than accreting a fourth name.

The alternative already explored and rejected on checker grounds (recorded in the `inputs`
decision): one raw source plus a `parse` codec, which failed only because expected types do not
flow through `map` bodies. That blocker is the same checker rework two other features are
already waiting on (see [input is typed by its position](#input-is-typed-by-its-position-and-only-some-positions-count)).

Changing this touches: every corpus case that reads stdin (about twenty files), the minimal
streaming cut, the `inputs` decision, and the streams decision. jq-inherited names, homegrown
exclusivity rules.

## `input` is typed by its position, and only some positions count

`input` has no type of its own; it is checked against whatever the position expects, first use
wins. But only positions the checker actually pushes expectations into. A record field is
synthesised, so:

```
fn f(v: Vec<Int>) -> Int = extent(v)

{a: f(input), b: input}
```

fails with "cannot tell what `input` contains" -- on the second `input`, three tokens after the
first one was happily typed. The same hole makes `[]` unusable even when the answer is in plain
sight:

```
fn nothing(x: Int) -> Vec<Int> = []
```

"cannot tell what `[]` contains", despite the return annotation. Function bodies are synthesised
and then compared, not checked.

This is the known checked-only-forms gap (research-log has it as a class), homegrown, and not a
decision anyone made so much as a rework nobody has done. It is listed here because it is now
the *third* feature blocked on the same fix: `parse` was rejected for it, enum literal ascription
(`"active" : Status`) is deferred on it, and the fragments above are what a user hits meanwhile.

Changing it breaks nothing decided; the draft twice names it as future work.

## Two calling conventions: subject-fed and argument-fed

`map`, `select`, and the match-arm chain take their data from the left of `|`; everything else
takes it as an argument:

```
map(. * 2)
```

"`map` needs a subject, so it must follow `|`". Meanwhile `extent([1, 2])`, `collect(...)`,
`str(...)` are ordinary calls. So the language has two ways a function receives its input, and
which one a given name uses is a fact to memorize. The draft's claim that `select` and `map` are
"not special syntax -- they are ordinary names" is half true: the parser no longer knows them,
but the checker reserves the names, rebinds `.` inside their argument, and refuses to let a user
define anything by these names. They are keyword-shaped builtins wearing call syntax.

jq-inherited idiom (jq filters read their input implicitly), homegrown mechanism. The
alternative the language itself already contains: arguments travel as records precisely so
several things can be passed, and the draft's own early sketch spelled it
`map {over: ..., with: ...}`. A subject-free spelling would also give `map` a value to fuse on
without pipe position mattering.

Changing this touches the 16 corpus cases calling `map(`, the select cases, the fusion pipeline
shape (fusion reads one chain from source to sink), and the records decision, which leans on
`map` being the only dimension-crossing form.

## Bare application: a third call spelling with zero users

Three ways to apply a function: `f(x)` anywhere, `f {record}` anywhere, and `f x` only where an
expression begins fresh. The bare form comes with its own exclusion set (`-` cannot start a bare
argument, uppercase names cannot be bare callees), its own parser flag, and its own failure
modes. Verified:

```
fn first_len(v: Vec<Str>) -> Int = extent v
```

"`extent` is not defined" -- the bare form is silently off inside a definition body's own
top-level chain (the juxtaposition-hole fix), so `extent` parses as a variable and the error
points at a name that is defined, with a message that says it is not. Same shape at the root:
`f -1` with `f` a defined function gives "`f` is not defined", because functions are not values
and the subtraction reading wins.

The inventory fact: the corpus and examples contain not one use of `f x`. Every real program
writes `f(x)` or `map {...}`. The feature's only users are its own unit tests
(tests/bare_application.rs). Homegrown, Haskell-flavored, and load-bearing for nothing.

Changing it breaks the decided section "`f x` reads as `f(x)`" and its test file, and no
program. Keeping it costs the two misleading diagnostics above plus the next item.

## The paren-free record argument reaches across the definition boundary

The record-argument sugar (`ident {` is a call) was decided as safe because "`{` cannot start an
expression and cannot follow one, so `ident {` is a syntax error today." That claim is now
false in exactly one place, and it is a place programs actually have:

```
fn id(s: Str) -> Str = s

{a: 1}
```

"expected an expression, found end of program". The definition's body `s` swallowed the
program's record-literal body as `s({a: 1})`. The bare-call form is suspended across this
boundary (that is what the parser flag does), but the `ident {` form is not, so the same
juxtaposition hole the research log documents for bare calls is open for record arguments.

Homegrown, and arguably a bug rather than a design question -- but the fix has options (suspend
`ident {` at the same boundary the bare form is suspended at; require parens in def bodies;
delimit def bodies) and picking one is a decision. No corpus case hits it because no corpus
program ends a definition body with a bare variable.

## `//` separates match arms

```
fn area_ish(s: Shape) -> Int = s | circle{r} -> r * r // point -> 0
```

Runs, and prints 9 for `circle{r: 3}`. To a C-family reader, everything after `//` is a comment;
to a Python reader it is integer division. The justification came from jq, where `//` means
"alternatives, left to right" -- but toylang has not implemented jq's `//` (the
default-on-absent operator), so today the token exists *only* between match arms, and the
heritage that motivated the spelling is not present to teach anyone what it means. The comment
character here is `#`, which softens the C-family collision but does not remove the reading.

jq-inspired spelling, homegrown use. Alternatives: a newline-terminated arm list, a keyword, a
different token. `,` was considered and rejected in the draft for a reason that still holds
(it is reserved for branch-wider semantics if the effect layer ever grows them).

Changing it touches six enum corpus cases, examples/shapes.toy, and the enum decision's
consumption syntax.

## A keep spec with nothing after it is refused, with the wrong message

```
[1, 2, 3][]
```

"`[]` must be followed by a field access". Two oddities in one. First, the rule: `v[]` is
"keep every entry," which is the identity on a `Vec`, and the language refuses it rather than
letting it mean what it means. The stated reason (a spec answers what an access does to a
dimension; no access, no question) is coherent but produces a form that is legal in the middle
of a chain and illegal at its end. Second, the message is false: an index is also legal after
`[]` -- `[[1, 2], [3]][][1]` runs and prints `[2,null]` -- and so is `!`.

Homegrown (the spec model). jq's `.[]` iterates here instead, which the design deliberately
dropped. Alternatives: allow `v[]` as the identity; keep the refusal and fix the message.
No corpus case is affected either way.

## Projection has two spellings

```
[{n: 1}, {n: 2}][].n        -> [1,2]
[{n: 1}, {n: 2}] | map(.n)  -> [1,2]
```

Verified identical. The draft acknowledges the pair and used it as the argument for giving
record assembly two spellings as well (which was refused). Both are load-bearing in the corpus:
ten files use `[].field`, sixteen use `map(`. jq has the same duplication (`.[].name` versus
`map(.name)`), so this is inherited -- and the issue's brief is explicit that inheritance is not
a defense. The cost is what every duplication costs: a reader must learn both to read other
people's programs, and style will fork.

Changing it means demoting one spelling, which touches whichever half of those corpus cases
used the demoted one, and (if `[].field` goes) the spec-model section that makes field
distribution over a kept dimension a central example.

## Naming: `extent`, `unlines`, `str`

Three names whose cost is paid by every newcomer, in exchange for internal consistency:

- `extent(v)` is `length`. The glossary bans length/size/cardinality with reasons ("how many
  entries a dimension has"), and the reasons are real, but the result is that the single most
  common collection operation in programming has a name no other language uses. 12 files use it.
- `unlines(v)` is Haskell's name, chosen because `lines` was "spoken for by the splitting
  direction that `stdin.lines` needs." What shipped instead is `lines` as a bare stdin keyword:
  not a splitter, not `unlines`'s inverse, not applicable to a `Str` at all. There is no
  string-splitting function in the language. So the pair that justified the name does not
  exist: `unlines` has no `lines` to be the un- of.
- `str(x)` reads as to-string and is Int-to-Str only. Verified: `str("a")` fails with
  "expected Int, found Str". The name promises polymorphism the function does not have; the
  identity case is the one that errors. 29 corpus files call it.

All homegrown. Alternatives: adopt the ordinary names and amend the glossary (it is
human-authored, so that is a human's call); or keep the glossary and accept the tax; or for
`str`, make it polymorphic (it is listed as the first candidate the moment another printable
type wants a string form). Changing `extent`/`str`/`unlines` renames across 11/29/9 corpus
files respectively plus prelude.toy, and touches CONTEXT.md's avoid-lists.

## Concatenation is spelled differently per type

```
"a" + "b"              -> "ab"
concat([[1, 2], [3]])  -> [1,2,3]
[1, 2] + [3]           -> `+` does not apply to Vec<Int>
```

All three verified. Strings concatenate with `+` (jq-inherited); Vecs concatenate through a
named builtin that is also not the binary `concat(a, b)` every other language means by the word,
but a unary flatten (jq's `add`, roughly). The refusal of `+` on Vecs is deliberate -- Q2
(cartesian/zip/explicit) is open and an overload would prejudge it -- so the asymmetry is a
hostage to an open question, not an accident. But it reads as one until you know that, and
`concat`'s arity surprise is independent of Q2.

Changing `+`-on-Vec in any direction decides Q2, which is the review session's biggest lever
here. Renaming or re-arifying `concat` is cheap: three corpus files.

## `<` on `Str` typechecks, and no two backends promise the same answer

```
"a" < "b"   -> true
```

Comparison dispatches on operand type and accepts `Str`. The draft itself records that the
three string representations (Lua bytes, JS UTF-16, native pointer+length) "agree on ASCII and
are not guaranteed to agree beyond it," and names `<` on `Str` as "where that surfaces first,
and it typechecks today." The corpus contains exactly one case, `str_ordering.yaml`, and it is
ASCII, so the agreement harness is structurally blind to the divergence until someone lands a
non-ASCII case, at which point some backend set starts failing on a semantics nobody chose.

jq compares strings (by codepoint), so allowing it is inherited; leaving the cross-backend
meaning unpinned is homegrown drift. Alternatives: pin an ordering (codepoint order is the
portable candidate) and add the non-ASCII corpus case that proves it; or refuse `<` on `Str`
until Q16 settles the representation. The first is a conformance rule, the second breaks one
corpus case.

## `Opt` can be produced but not written

```
fn head(v: Vec<Int>) -> Opt<Int> = v[0]
```

fails at *parse* time: "expected `=`, found `<`". `Opt` is not in the type grammar at all --
only `Vec` and `Stream` take parameters -- so a function can return an `Opt` value's unwrapped
form or nothing. Every `Opt` a program meets was made by an index and must be consumed by `!`
more or less immediately. The knock-on cost is visible in the one function the prelude has:

```
pub fn unlines(v: Vec<Str>) -> Str =
    "" if extent(v) == 0 else v[0]! if extent(v) == 1 else v[0]! + "\n" + unlines(tail(v)!)
```

Three `!`s in one line, two of them structurally incapable of failing (the branch conditions
already proved the elements exist), because `Opt` cannot flow through a signature and `tail`
returns `Opt<Vec<T>>` (verified: `tail(range(0))` prints `null`) rather than an empty Vec.

Homegrown, and named in the draft as "the next step" -- this entry is here because the tax is
already being paid in shipped code, not because the gap is unknown. Additive to fix; breaks
nothing.

## `!` and `!=` are one space apart

```
[1, 2][0]!=1
```

lexes as `!=` and fails with "expected Opt<Int>, found Int". The unwrap-then-compare a reader
might intend is spelled `[1, 2][0]! == 1`. Postfix `!` next to a C-family `!=` token makes
whitespace load-bearing in exactly one spot. The type system catches every such confusion today
(an `Opt` never compares equal-typed against its element), so this is a diagnostics oddity
rather than a correctness one -- but the error message points at types when the actual mistake
is lexical.

Homegrown (Swift lives with the same clash). Alternatives: a different unwrap spelling; a
targeted diagnostic ("did you mean `! ==`?"); nothing. Five unwrap corpus files use `!`.

## Record fields print in sorted order, so field order is not data

```
{b: 1, a: 2}   -> {"a":2,"b":1}
```

The printer enumerates fields from the type, sorted, which is what keeps seven backends from
disagreeing about key order -- a real reason. The consequence: toylang is a JSON-transformation
language whose output key order is the sort order, not the written order, where jq preserves
insertion order by default (`-S` is opt-in). `{message, name}` in the jq tutorial reproduction
survives only because m happens to sort before n. Anyone whose downstream diffing or golden
files care about key order gets alphabetical whether they asked or not, and the draft elsewhere
lists "object key ordering if JSON round-tripping is meant to be stable" as an open platform
concern, unconnected to this already-made choice.

Homegrown. Alternatives: carry declaration order in the record type (the type already carries
names; order is one more bit of the same kind); document sorted order as the language's
canonical form and close the round-tripping question against it. Changing it touches every
record-printing corpus expectation (about 27 files) and the field-index invariant shared by the
struct-of-arrays layout and the Go/native emitters, which currently equate a field's sorted
position with its column.

## `jsonlines` is a call-shaped statement

```
[jsonlines([1, 2])]
```

"`jsonlines` is a sink, legal only as the program's outermost expression". The one construct in
the language with no result type wears the same syntax as every value-producing call, and its
position rule (outermost only) is invisible until violated. The type story underneath is honest
and deliberate (a sink's result cannot be observed, so it has none, and Q35 stays open); the
oddity is purely that the surface gives no hint that `jsonlines` is a different kind of thing
from `unlines` -- one letter of prefix apart, one a pure function, one a program form.

Homegrown. Alternatives: a syntactically distinct sink position (a final `|> jsonlines` stage,
a keyword); leave it and let the error teach. Renaming or resyntaxing touches the eight
jsonlines corpus files and the streams decision's sink rules, and anything chosen here should
avoid prejudging Q35 (what stdout is), which the current design carefully does not.
