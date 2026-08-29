# A JSON parser in the matcher algebra: the spike

Commissioned in the matcher-totality round (kantord/toylang#47, third comment; filed as #71):
build a complete JSON parser in the proposed matcher algebra, invent the syntax, and name each
version's problems honestly, so the joint review round can weigh real programs instead of
principles.

**Status of every program in this file: proposal.** Nothing compiles. Nothing is expected to
compile. Every name that is not in the language today (`Parsed`, `Seq`, `Star`, `lit`,
`one_of`, `text`, `join`, `unicode`, the whole `syntax` block) is invented here, in this file,
for the purpose of being argued with. Where a name looks like an API, it is an unverified
sketch of one.

The ratified constraints this spike works inside, from #47's comment history:

- Matchers are first-class tagged values; a non-match is a value `or` can compose.
- Capital name means derived matcher (`Foo` reads as `is_foo`); matchers are not
  human-definable, so only derived matchers start capitalized.
- `Alt()` is the alternation combinator, uppercase.
- Exhaustiveness lives in the type-named application form, `Status( Active -> "on" or ... )`.
- A parser must thread position and remaining input, which is exactly where the
  matcher-as-`fn(T) -> tagged` shape may strain. The commission asks where it breaks.

It breaks in one specific place, and both versions below hit it: **the ratified tag answers
"did it match", and a parser's yes must also answer "and here is what is left".** Everything
else in this file is downstream of how each version smuggles that second answer through.

## The target type, and the floor both versions stand on

Both parsers produce the same value, so the differences between them are entirely in the
parsing surface:

```
# PROPOSAL (but the closest thing in this file to something that could land as-is)
enum Json {
  jnull,
  jbool{b: Bool},
  jnum{raw: Str},
  jstr{s: Str},
  jarr{items: Vec<Json>},
  jobj{members: Vec<{key: Str, val: Json}>},
}
```

Two honest notes on the target before any parser exists:

**This is the language's first recursive type.** `jarr` carries `Vec<Json>` inside `Json`.
Generic enums landed in #62 but nothing exercises a self-referential payload; whether the
checker's enum registry survives one is untested. And the parser itself is a family of
mutually recursive functions (`value` calls `array` calls `element` calls `value`), while the
draft's parallel-basis section ("The primitive set cannot be fold and recursion") explicitly
refuses general recursion as a standard-library basis. Named functions may recurse (the
annotation requirement was justified partly by recursion), but a JSON parser is the first
program that *needs* it, and it is sequential to the bone. If the answer is "parsers are
allowed to be sequential", that is a real carve-out from the vectorization stance and should
be recorded as one.

**`jnum{raw: Str}` is an admission.** The language has `Str`, `Int`, `Bool`, and composites.
JSON numbers have fractions and exponents; there is no float type, and `Int` is 32-bit (the
Euler stream's finding, and the Int64 decision is still pending). So this parser *recognizes*
the complete RFC 8259 number grammar but stores the lexeme, because the parsed value has no
type to live in. A parser that returned `Int` would be complete over a dialect, not over JSON.

Completeness means RFC 8259 / json.org: `null`, `true`, `false`, numbers with optional sign,
fraction, and exponent; strings with the two-character escapes, `\uXXXX`, and the control-char
exclusion; arrays and objects, empty and not, with whitespace in every legal position.

Holes in the floor, shared by both versions because they are missing primitives rather than
algebra problems:

- No `Char` type and no character ranges. "Any character except `"`, `\`, and the 32 control
  characters" is unrepresentable by enumeration; both versions need a new primitive for it.
- No `Str` operations to speak of: no join of `Vec<Str>` into `Str`, no hex-digits-to-int, no
  codepoint-to-character. Spelled `join()` and `unicode()` below, both invented.
- Whether toylang string *literals* support `\u0008`-style escapes is unverified; the escape
  table below assumes they do.

## Version A: cursor-explicit combinators

The conservative stretch: take the ratified algebra literally and see what a parser costs.
A matcher is `fn(T) -> tagged`; a parser is the same shape where the yes-tag carries the pair
the introduction named:

```
# PROPOSAL
enum Parsed<T> { hit{value: T, rest: Str}, miss }
```

A parser is any `fn(Str) -> Parsed<T>`. Grammar rules are ordinary named functions. The
existing arm syntax is reused with one extension, which is the ratified first-classness taken
at its word ("the left side of an arm is a function call"): the left side of an arm may be any
parser, not only a variant matcher. Piping a `Str` into an arm chain runs each parser in
order, first `hit` wins, and `.` in the body is the hit's *value* (the rest is threaded
invisibly; that invisibility is problem one below).

Invented primitive parsers, lowercase because they are constructors of matcher values, exactly
as `circle{r: 1}` is a lowercase constructor of a `Shape`:

- `lit(s)`: exactly the text `s`, yielding it.
- `one_of(chars)`: any single character enumerated in `chars`, yielding it.
- `eof`: end of input, yielding `""`.

Invented combinators, uppercase because `Alt()` was ratified uppercase and these are its
siblings (a capital applied to matchers yields a matcher):

- `Alt(p, q, ...)`: ordered choice; a miss consumes nothing. The `or` between arms is sugar
  for it.
- `Seq{a: p, b: q, ...}`: sequencing as a record of parsers, run in field order, yielding the
  record of their values. This is the draft's own correspondence ("a record pattern's fields
  in sequence is `Seq`") taken literally.
- `Star(p)`: zero or more, yielding `Vec`. `Plus(p)`: one or more. `Rep(n, p)`: exactly `n`.
- `Maybe(p)`: zero or one, yielding `Opt`.
- `text(p)`: run `p`, discard its value, yield the consumed text instead. Lowercase, because
  it is how a lexeme-level rule opts out of building structure. (nom calls this `recognize`;
  pest gets it from spans.)

The complete parser:

```
# PROPOSAL: none of this compiles

fn json(s: Str) -> Parsed<Json> =
  s | Seq{lead: ws, v: value, tail: ws, fin: eof} -> .v

fn value(s: Str) -> Parsed<Json> =
  s | lit("null")  -> jnull
    or lit("true")  -> jbool{b: true}
    or lit("false") -> jbool{b: false}
    or number       -> jnum{raw: .}
    or string       -> jstr{s: .}
    or array        -> .
    or object       -> .

fn ws(s: Str) -> Parsed<Str> =
  s | Star(one_of(" \t\n\r")) -> join(.)

# Numbers: recognized in full, stored as the lexeme (see the jnum note above).

fn digit(s: Str) -> Parsed<Str> = s | one_of("0123456789") -> .
fn hex(s: Str)   -> Parsed<Str> = s | one_of("0123456789abcdefABCDEF") -> .

fn int_part(s: Str) -> Parsed<Str> =
  s | lit("0") -> .
    or text(Seq{lead: one_of("123456789"), rest: Star(digit)}) -> .

fn number(s: Str) -> Parsed<Str> =
  s | text(Seq{
        sign: Maybe(lit("-")),
        int:  int_part,
        frac: Maybe(Seq{dot: lit("."), digits: Plus(digit)}),
        exp:  Maybe(Seq{mark: one_of("eE"), sign: Maybe(one_of("+-")), digits: Plus(digit)}),
      }) -> .

# Strings.

fn escape(s: Str) -> Parsed<Str> =
  s | lit("\"") -> "\""
    or lit("\\") -> "\\"
    or lit("/")  -> "/"
    or lit("b")  -> "\u0008"
    or lit("f")  -> "\u000c"
    or lit("n")  -> "\n"
    or lit("r")  -> "\r"
    or lit("t")  -> "\t"
    or Seq{u: lit("u"), h: text(Rep(4, hex))} -> unicode(.h)

# `unescaped` cannot be written in this algebra: it is "any character except quote,
# backslash, and the 32 control characters", and the primitives enumerate characters but
# cannot complement or range over them. Irreducibly builtin until that hole closes.
fn unescaped(s: Str) -> Parsed<Str> = ...

fn string_char(s: Str) -> Parsed<Str> =
  s | Seq{mark: lit("\\"), e: escape} -> .e
    or unescaped -> .

fn string(s: Str) -> Parsed<Str> =
  s | Seq{open: lit("\""), chars: Star(string_char), close: lit("\"")} -> join(.chars)

# Arrays and objects. `element` is json.org's `ws value ws`; the comma-tail helpers exist
# because an arm (`p -> body`) used as an argument to Star would need its own precedence
# story, so each gets a name instead.

fn element(s: Str) -> Parsed<Json> =
  s | Seq{lead: ws, v: value, tail: ws} -> .v

fn element_tail(s: Str) -> Parsed<Json> =
  s | Seq{comma: lit(","), e: element} -> .e

fn elements(s: Str) -> Parsed<Vec<Json>> =
  s | Seq{head: element, tail: Star(element_tail)} -> concat([[.head], .tail])

fn array(s: Str) -> Parsed<Json> =
  s | Seq{open: lit("["), items: elements, close: lit("]")} -> jarr{items: .items}
    or Seq{open: lit("["), sp: ws, close: lit("]")}         -> jarr{items: []}

fn member(s: Str) -> Parsed<{key: Str, val: Json}> =
  s | Seq{lead: ws, key: string, tail: ws, colon: lit(":"), val: element}
      -> {key: .key, val: .val}

fn member_tail(s: Str) -> Parsed<{key: Str, val: Json}> =
  s | Seq{comma: lit(","), m: member} -> .m

fn members(s: Str) -> Parsed<Vec<{key: Str, val: Json}>> =
  s | Seq{head: member, tail: Star(member_tail)} -> concat([[.head], .tail])

fn object(s: Str) -> Parsed<Json> =
  s | Seq{open: lit("{"), ms: members, close: lit("}")} -> jobj{members: .ms}
    or Seq{open: lit("{"), sp: ws, close: lit("}")}     -> jobj{members: []}
```

That is the whole grammar. It reads like the language: the arm chains are the enum-match
shape from the corpus, `.` rebinding is the existing rule, constructors are lowercase, the
combinators wear `Alt()`'s casing. The problems are real, though, and most of them are the
threading question wearing different costumes.

### Where Version A strains

**The rest is invisible, and the ratified tag has no slot for it.** Look at any arm:
`number -> jnum{raw: .}`. The `.` is the number's lexeme. Where is the remaining input? It
is inside the `hit` that the arm machinery unpacked, silently, and handed forward to the
next parser in the `Seq`, silently. The ratified matcher shape (`fn(T) -> tagged`, non-match
composable by `or`) never had to answer this because a variant matcher consumes nothing: its
subject is whole, its answer is whole. The moment matching *consumes*, the tagged result must
carry a pair, and there are only two places to put it. Either `Parsed<T>` is a second tagged
type living beside `Matcher` (so the algebra is duplicated: `or` over matchers and `or` over
parsers obey different laws while looking identical), or the `Matcher` tag itself widens to
carry a rest that variant matchers never use (so every match against a plain enum pays rent
on a field that is only meaningful over streams of input). This is the break the commission
predicted, located. It is not fatal, but whichever way it is resolved should be resolved on
purpose, in the review round, not inherited from whichever spelling ships first.

**`or` needs a rewind law it never needed before.** `array` has two alternatives that both
begin by consuming `[`. When the first alternative dies at `elements`, the second must see
the input rewound to before the `[`, or `[ ]` can never parse. For variant matchers,
or-composition needed no consumption law because nothing consumed. Committing to "a miss
consumes nothing" is committing to PEG-style ordered choice with unbounded rewind, and its
costs come with it: `text(Seq{...})` in `number` can walk arbitrarily far before dying, and
a grammar author has to know that `or` retries from the left edge, not from the failure
point. The alternative (no rewind after consumption, as in early parser combinators without
`try`) makes `array` above silently wrong. Either law is defensible; the ratified surface
currently implies neither, and a parser cannot be written without picking one.

**The capital namespace stops carrying information.** The ratification says capital means
derived matcher and matchers are not human-definable. Every rule in this parser (`value`,
`number`, `string`, `array`) is a human-written lowercase function used in matcher position.
Result: in the one program where matchers are the whole point, almost nothing is capitalized
except the seven combinators. Either grammar rules deserve capitals (which repeals
"not human-definable", since a parser author mints matchers by the dozen), or the casing
rule's real content shrinks to "capitals mark the compiler's own derivations", and parser
code reads as an exception the reader must learn. The spike leans toward the honest repeal:
`Alt(Foo, Bar)` reading as `is_alt_foo_bar` already treats matcher-composition as
name-composition, and `value` here *is* `is_json_value` in every sense that matters.

**Record-as-Seq forces naming the junk.** `Seq{open: lit("["), sp: ws, close: lit("]")}`:
three fields, zero of which any body reads. Record fields must be named and unique, so the
grammar is strewn with `open`, `close`, `comma`, `colon`, `lead`, `tail`, `mark`, `sp`,
invented purely to satisfy the record shape. Roughly half the tokens in the object and array
rules are this noise. A positional `Seq(p, q, r)` with positional yield access would cut it,
at the price of losing the named-capture readability that makes `member` legible. Both
spellings should go to the review round; the record spelling is shown because it is the one
the draft's own `Seq`-is-a-record correspondence implies.

**The totality machinery has nothing to do here.** The form ratified hardest in #47 (the
type-named application `Status( Active -> ... or ... )`, coverage checked against the closed
variant set) never appears above, because no parse over `Str` is total: unexpected input is
always a live variant nobody can enumerate. The entire parser lives in the open-world,
guard-chain half of the match design. That is not a defect, but it is a finding: the
exhaustiveness boundary and the parser algebra share syntax and share nothing else. Q30's
"the matchers build parser combinators" claim is true of the composition operators and false
of the totality form, and the review round should ratify it at that reduced strength.

**Generic helpers may not be expressible.** `elements` and `members` are the same function
at two types (`sep_by`, in combinator-library terms). Whether user functions can be generic
over a type parameter is undecided (generic *enums* landed in #62; generic `fn` did not), so
they are written out twice. If generic functions never land, every combinator library
pattern (`sep_by`, `between`, `chainl`) is spelled per-type by hand, and Version A's cost
scales with grammar size in a way real combinator libraries do not.

## Version B: the grammar block

The aggressive stretch: if types, constructors, and codecs are one thing (the draft's own
direction), then a grammar is a *declaration*, and threading input is the compiler's job,
never visible in user code. Version B parses JSON with zero cursors, zero `Parsed`, zero
threading, at the price of a new declaration form with its own interior rules.

Inside a `syntax` block: juxtaposition is sequencing; `or` is ordered choice with the same
rewind law as Version A (sugar cannot dodge that decision, only relocate it); postfix `*`,
`+`, `?` are repetition; a bare string literal is `lit`; `[...]` is a character class with
ranges and complement; `name:rule` captures a rule's yield under a fresh binding for the
arm body; `$(...)` yields the matched text instead of structure; a rule with no arrow yields
its matched text. Yes, that is eight new meanings. That is Version B's price tag, itemized
below.

```
# PROPOSAL: none of this compiles, and the `syntax` form itself is invented whole

syntax json -> Json {
  json    = ws v:value ws eof                  -> v

  value   = "null"                             -> jnull
         or "true"                             -> jbool{b: true}
         or "false"                            -> jbool{b: false}
         or n:number                           -> jnum{raw: n}
         or s:string                           -> jstr{s: s}
         or a:array                            -> a
         or o:object                           -> o

  array   = "[" ms:elements "]"                -> jarr{items: ms}
         or "[" ws "]"                         -> jarr{items: []}

  elements = h:element t:("," e:element -> e)* -> concat([[h], t])
  element  = ws v:value ws                     -> v

  object  = "{" ms:members "}"                 -> jobj{members: ms}
         or "{" ws "}"                         -> jobj{members: []}

  members = h:member t:("," m:member -> m)*    -> concat([[h], t])
  member  = ws k:string ws ":" v:element       -> {key: k, val: v}

  string  = "\"" cs:char* "\""                 -> join(cs)
  char    = "\\" e:escape                      -> e
         or c:[^ "\"" "\\" "\u0000".."\u001f"] -> c

  escape  = "\""                               -> "\""
         or "\\"                               -> "\\"
         or "/"                                -> "/"
         or "b"                                -> "\u0008"
         or "f"                                -> "\u000c"
         or "n"                                -> "\n"
         or "r"                                -> "\r"
         or "t"                                -> "\t"
         or "u" h:$(hex hex hex hex)           -> unicode(h)

  hex     = ["0".."9" "a".."f" "A".."F"]

  number  = $(sign? int frac? exp?)
  sign    = "-"
  int     = "0" or ["1".."9"] digit*
  frac    = "." digit+
  exp     = ["e" "E"] ["+" "-"]? digit+
  digit   = ["0".."9"]
}
```

The whole grammar again, at well under half Version A's length, with the junk names gone
(unnamed sequence parts simply are not captured) and the control-character exclusion
expressible (`[^...]` with ranges). The right sides of the arms are ordinary expressions
building ordinary values; the bindings are fresh names, which is the existing record-pattern
convention (`circle{r} -> r * r` already binds `r` fresh), not a new one. Prior art it
borrows knowingly: PEG's ordered choice, pest and PEG.js for `$()` and character classes,
Ohm for grammar-as-declaration.

### Where Version B strains

**It is a second language, behind a keyword, and the border leaks.** Inside the block,
juxtaposition means sequence. Outside, juxtaposition already means application:
`describe ping` is a pinned corpus case (`call_without_parens.yaml`). The same token stream
means different things depending on which side of a `{` the reader stands. String
literals flip from values to commands. `or` keeps its arm meaning but silently gains the
rewind law. A reader cannot apply what they know of the language inside the block, or what
they learn in the block outside it, and the arm bodies (which *are* ordinary expressions)
sit embedded in grammar-land, so the mode switch happens per-line, twice per line.

**It reverses a trade the draft already recorded.** The string-pattern discussion chose
named combinator calls over metacharacters (`mul("a")` rather than `a*`, explicitly, "the
same trade this document already made for and/or/not") to avoid a second syntax to learn.
Version B reinstates `*`, `+`, `?`, `$`, `[^...]`. If Version B wins the review round, that
recorded decision needs to be reversed on the record, not quietly contradicted by a new
form.

**The block is more expressive than the language it lives in.** Character ranges and
complement exist only inside `syntax`. So either the value-level algebra grows the same
primitives (at which point Version B is pure sugar over Version A, and every Version A
problem still exists underneath, merely hidden), or grammar-land can recognize things
value-land cannot, and Q30's "one algebra across trees, strings, and streams" claim is
false at the exact boundary this spike was commissioned to test. There is no third option,
and the review round should say which one it is buying.

**Rules are not values.** The ratified first-classness (matchers are first-class, arm chains
are ordinary combinator composition) is precisely what the block gives up. `elements` and
`members` are copy-paste of each other *in the code above*, and inside the block nothing can
abstract them, because a rule cannot be passed to a function, returned, or parameterized.
Version A at least might grow generic functions and fold that duplication; Version B's
duplication is structural. A grammar author gets exactly the combinators the block grammar
hard-codes, forever.

**`name:` acquires a fourth meaning.** Record literal field, record pattern field, type
annotation, and now capture. The `member` rule has capture-colons and a record-literal colon
in one line (`k:string` against `{key: k, ...}`), plus JSON's own `":"` as a matched token
between them. It parses unambiguously; it reads like a colon convention that grew rather
than one that was designed.

## The alternative deliberately not written out: the grammar is the types

There is a third candidate the commission's "consider all the alternatives" covers, visible
from the draft's codec trinity: no parser code at all. Declare types whose *shape* is the
grammar (`Vec` as repetition, `Opt` as optionality, enum as alternation, record as
sequencing, per Q30's correspondence) and derive the `Str -> T` parse codec the way the
`Json -> T` decode was argued to be derivable.

It is not written out because it dies in the first rule. A type can say *shape* but not
*surface*: nothing in `jarr{items: Vec<Json>}` says `[`, says `,`, says whitespace may
appear here, or says alternatives are tried in this order (enum variant order has never
been semantically load-bearing, and ordered choice would make it so, invisibly). Every fix
is an annotation attaching surface syntax to the declaration, and by the third annotation
the "derived" grammar is Version B wearing the type declaration as a costume. The honest
version of this idea *is* Version B. What survives of it: the target `Json` enum plus a
derived encoder is the right story for the *other* direction (printing), where shape is
genuinely all there is.

## What the review round has to decide

The two versions disagree on surface but force the same five decisions, and the spike's
value is that each now has a concrete program to point at:

1. **Where the rest lives.** A second `Parsed` type beside `Matcher` (duplicated algebra),
   or a widened `Matcher` tag (variant matches carry a dead field). Version A's first
   strain, and it exists under Version B too, one layer down.
2. **The rewind law for `or` over consuming matchers.** Full PEG rewind (the spike's
   assumption, required by both versions' `array`) or explicit-commit semantics. Must be
   stated as a law of the algebra either way.
3. **Whether humans may mint capitals.** Repeal "matchers are not human-definable" so
   grammar rules join the capital namespace, or accept that parser code is where the casing
   rule goes dark.
4. **Value-level or declaration-level.** Version A's shape (everything is the language,
   threading and junk names visible) against Version B's (grammar legible, at the cost of a
   mode switch, a reversed decision, and rules that are not values). A sugar-block-over-A
   compromise inherits both price tags and should be argued as such, not assumed free.
5. **The floor.** Character classes with ranges and complement, `join`, `unicode`, a numeric
   type wide enough for JSON numbers, and a ruling on recursive enums and mutually recursive
   functions. Every one is prerequisite to either version; none is matcher design.
