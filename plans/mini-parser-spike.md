# A tiny parser, no invented syntax: the second spike

Commissioned by kantord/toylang#77, the maintainer's parallel track from the parser-review
round (#47) while the missing floor gets built: spike a parser for a language small enough to
need none of it, testing the round's sharpened surface answer -- Version A direction, but
**no parser-specific syntax; whatever the parser uses must be generic language machinery** --
end to end, in code that actually runs.

**Status of every program in this file: it runs.** Every listing below was executed with
`toylang run` against six of the seven backends (lua, js, py, go, rust, llvm) with matching
output; the jq backend hits a real, pre-existing gap this spike is the first program to
trigger, covered in its own section below. Where
[matcher-parser-spike.md](matcher-parser-spike.md) (the JSON spike, #71) is proposal from the
first line, this file is the opposite case: nothing here needed a `# PROPOSAL` tag, and that
gap between the two spikes is itself the finding.

## The language: arithmetic expressions over tokens, not text

The JSON spike's holes in the floor were concrete: no `Char` type or character ranges, no
`Str` join or codepoint conversion, an untested recursive enum (`Vec<Json>` inside `Json`).
Avoiding all three at once rules out lexing free-form text -- any scan of raw `Str` input
needs the character-class primitives that spike found missing. So this spike does not parse
`Str`. It parses `Vec<Token>`, tokenizing assumed already done (a separate, smaller problem
than this one, and not what #77 commissions):

```
enum Token { num(Int), plus, minus, star, slash, lparen, rparen }
```

Standard arithmetic, left-associative, with parentheses and unary minus:

```
expr   = term (('+' | '-') term)*
term   = factor (('*' | '/') factor)*
factor = num | '(' expr ')' | '-' factor
```

Evaluated directly to `Int` rather than built into an AST -- the JSON spike's other open
question, whether a self-referential enum survives the checker, stays open. This spike closes
the syntax question, not that one.

## The parser

`Parsed<T>` is the same shape the JSON spike proposed: a plain generic enum, two variants, no
new declaration form. Unlike that spike, it needed no `Seq{}` or arm-chain-on-parsers
extension to write against -- everything below is a call, a pipe, a match over an ordinary
enum's own variants, or `if`/`else`.

```
enum Parsed<T> { hit{value: T, rest: Vec<Token>}, miss }

fn factor(ts: Vec<Token>) -> Parsed<Int> =
  miss if extent(ts) == 0 else
  (ts[0]! | num    -> hit{value: ., rest: tail(ts)!}
          or lparen -> group(tail(ts)!)
          or minus  -> (factor(tail(ts)!) | hit{value, rest} -> hit{value: -value, rest: rest}
                                           or miss -> miss)
          or any()  -> miss)

fn group(ts: Vec<Token>) -> Parsed<Int> =
  expr(ts) | hit{value, rest} ->
               (miss if extent(rest) == 0 else
                (rest[0]! | rparen -> hit{value: value, rest: tail(rest)!}
                          or any() -> miss))
           or miss -> miss

fn term_rest(st: {value: Int, rest: Vec<Token>}) -> Parsed<Int> =
  hit{value: st.value, rest: st.rest} if extent(st.rest) == 0 else
  (st.rest[0]! | star -> (factor(tail(st.rest)!) | hit{value, rest} -> term_rest({value: st.value * value, rest: rest})
                                                  or miss -> miss)
               or slash -> (factor(tail(st.rest)!) | hit{value, rest} -> term_rest({value: st.value / value, rest: rest})
                                                    or miss -> miss)
               or any() -> hit{value: st.value, rest: st.rest})

fn term(ts: Vec<Token>) -> Parsed<Int> =
  factor(ts) | hit{value, rest} -> term_rest({value: value, rest: rest})
             or miss -> miss

fn expr_rest(st: {value: Int, rest: Vec<Token>}) -> Parsed<Int> =
  hit{value: st.value, rest: st.rest} if extent(st.rest) == 0 else
  (st.rest[0]! | plus -> (term(tail(st.rest)!) | hit{value, rest} -> expr_rest({value: st.value + value, rest: rest})
                                                or miss -> miss)
               or minus -> (term(tail(st.rest)!) | hit{value, rest} -> expr_rest({value: st.value - value, rest: rest})
                                                  or miss -> miss)
               or any() -> hit{value: st.value, rest: st.rest})

fn expr(ts: Vec<Token>) -> Parsed<Int> =
  term(ts) | hit{value, rest} -> expr_rest({value: value, rest: rest})
           or miss -> miss
```

Run against `2 + (3 * (4 - 1))` (as `Token` values, since there is no lexer here) this
evaluates to `11`, matches on the six backends that accept it, and `(1 + 2` (unbalanced) and
`[]` (empty input) both come back `miss` rather than refusing or crashing.

`expr_rest` and `term_rest` are the standard recursive-descent trick for left-associative
binary operators: `expr = expr '+' term` would recurse on `expr` before consuming anything, so
the repetition is written as a tail-recursive fold over an accumulator instead of transcribed
from the grammar directly. This is not a toylang-specific workaround -- every recursive-descent
parser in every language does this -- but it is worth naming because it is the shape "no
parser-specific syntax" forces: the language gives you recursive named functions and nothing
else, so that is what repetition compiles to by hand.

## What this proves runs today, that the JSON spike could not test

- **Mutual recursion between more than two named functions**, through `expr`, `term`,
  `factor`, and `group` (`factor` calls `group` calls `expr`), forward-referenced before their
  definitions appear.
- **A user-defined generic enum matched by its own variants**, nested two levels deep
  (`term_rest` matches `factor`'s `Parsed<Int>`, and inside the `star` arm's body matches it
  again). This is unrelated to the restriction on matching `Opt` by variant
  (`src/check/mod.rs`'s `variant_arm`, still refused pending the totality round) -- that
  restriction is Opt-specific; a plain user enum like `Parsed<T>` was never in question, and
  this spike is the first corpus-adjacent program to actually match one this deeply.
- **`if`/`else` as the sentinel branch of a match-like decision** (`miss if extent(ts) == 0
  else ...`), standing in for the wildcard-arm idiom (`or any() -> miss`) where the condition
  is a length check rather than a variant.
- **A record type as a de facto multi-field return and multi-field parameter**
  (`{value: Int, rest: Vec<Token>}`), doing double duty as `hit`'s payload shape and as
  `term_rest`/`expr_rest`'s single parameter.

## A real bug this spike surfaced, unrelated to the matcher algebra

`factor` calls `group` calls `expr` calls `term` calls `factor`: a four-function cycle, true
mutual recursion between named functions rather than a function calling itself. Six backends
run it correctly. The jq backend does not -- `jq: error: v_group/0 is not defined` -- because
jq's `def` only sees definitions already in scope (earlier in the same program, or itself, for
direct self-recursion); there is no forward declaration and no hoisting across a cycle. This
is not a defect in the parser or the matcher algebra. `src/emit_jq.rs`'s own `ordered()`
function documents the gap already: "Mutual recursion has no ordering that works and no
forward declaration to fall back on, so it is unrepresentable here rather than merely awkward.
Nothing in the language can express it yet, and this returns definitions unsorted rather than
looping if that changes." That comment was true until this spike: `term_rest` and `expr_rest`
recurse on themselves, which every other corpus program with recursion also does, but nothing
before this file seems to have chained four named functions into a cycle. Filed as
kantord/toylang#79 rather than fixed here, since fixing it means touching `src/`, outside this
spike's `plans/`-only footprint, and the fix (detect the cycle and refuse cleanly, versus
actually supporting it) is a real decision between two costs, not a one-line patch.

## Where it strains

**Functions are not first-class values, and that -- not the `Str` primitives -- is what
actually blocks Version A's combinators.** `fn apply(f: Int) -> Int = f(5)` given a bare
function name (`apply(double)`) fails at the call site with "`f` is not a function": a named
function can be *called*, not passed. Combined with every `fn` taking at most one parameter
(`fn apply(f: Int, x: Int)` is a parse error), `Alt(p, q)`, `Seq{...}`, and `Star(p)` -- the
JSON spike's proposed combinators, and Version A's whole appeal over hand-written descent --
cannot be written as reusable functions today, at any type. This spike does not use them:
`term_rest` and `expr_rest` are two independent, near-identical functions (one for `*`/`/`,
one for `+`/`-`) because there is no way to write the shared shape once and apply it twice.
The JSON spike flagged a narrower version of this same gap ("generic helpers may not be
expressible" -- `sep_by`, blocked on generic *functions*); this spike shows the gap is wider
than generics. Closures or passable named functions are a harder prerequisite for
Version A than any single character-class primitive, and belong in the floor's decided-first
queue in their own right, not folded into the generics question.

**The JSON spike's five open questions turn out to be avoidable, not answered, once
combinators are gone.** Restated against this program:

1. *Where the rest lives* -- `Parsed<T>` is an ordinary generic enum, no widened `Matcher`
   tag, no duplicated `or`-algebra. But this is only uncontested because nothing here ever
   composes two *parsers* with `or`; every choice is a match over `Token`'s own variants,
   which already have somewhere to put a payload. The question reopens the moment a
   combinator needs to hold a `Parsed` value's "or-ness" the way `Alt` would.
2. *The rewind law* -- also moot here, for a sharper reason than "nothing was mutated": every
   choice in this grammar decides on the *very next token* (`ts[0]`), so there is never a
   multi-token attempt to unwind. `group`'s call into `expr` after consuming `(` has no
   alternative to retry if it fails -- an unclosed paren is just `miss`. The grammar is
   LL(1) after the standard left-factoring (`expr`/`term`/`factor`), which is exactly the
   shape that never needs backtracking. A grammar with two productions sharing a longer
   common prefix would still need an answer this spike does not exercise.
3. *Whether humans may mint capitals* -- does not arise. Every pattern here is a `Token`
   variant name, matched the same way `enum_match_default.yaml` already matches `Msg`; no
   combinator value needed a name, capital or otherwise.

Items 1 through 3 held for the JSON spike's Version A specifically because it wrote `Alt`,
`Seq`, and `Star` as things. Take those away -- as "no parser-specific syntax" does -- and a
tiny grammar sidesteps all three, but pays for it in the duplication named above. The review
round should read this as two designs with different floor prerequisites, not one design at
two scales: hand-written recursive descent needs nothing beyond what shipped in #62 and #66;
a real combinator library needs first-class functions on top.

## What stayed out of scope on purpose

- **The `Result<T, E>` / `MatchResult` tri-state question (#72) is unsettled**, so `Parsed<T>`
  here keeps the JSON spike's two-state `hit`/`miss` shape rather than adopting the
  maintainer's three-state sketch. Folding that in would answer a question this spike was not
  commissioned to answer.
- **No recursive enum type.** Evaluating straight to `Int` sidesteps the JSON spike's
  untested `Vec<Json>`-inside-`Json` question entirely; it is still open, for whichever spike
  or corpus case picks it up.
- **No lexer.** The gap between `Str` and `Vec<Token>` is exactly the character-class /
  `join` / `unicode` floor the JSON spike named as missing; this spike assumes it is filled
  elsewhere rather than re-deriving it.
