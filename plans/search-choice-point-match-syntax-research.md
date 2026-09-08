# Choice-point syntax: should "branch wider" reuse match syntax?

Research spike for the search-choice-point-syntax question in the search-vocab-choice-cut-fold
round: the maintainer asked whether toylang's choice-point operator ("branch wider, explore every
alternative" -- currently sketched as an infix operator such as `;`, `branch(...)`, or folding
into `//`) should instead be spelled parallel to toylang's existing match syntax. The draft's
search vocabulary is recorded in [Query is search](../draft.md#query-is-search): `|` binds,
`,` is the draft's current choice-point sketch, `empty` dead-ends, `first` cuts, `..`
traverses, `select` prunes, and `//` was alternatives-left-to-right. This spike surveys how
Prolog, Icon/Curry, and the Haskell list monad spell this kind of nondeterministic branching,
then renders one worked example under three concrete toylang candidates, one of them reusing
match syntax as the maintainer asked for.

## What "parallel to match" would mean

Match is an `or`-joined chain of `pattern -> body` arms over the subject `.`, first-match-wins
([match reference](../docs/reference/operators/match.md). The chain's shape is load-bearing:
each arm is a produce-or-decline clause, arms compose left-to-right, and the first arm that accepts
wins. A choice point "spelled parallel to match" would therefore be the same chain with the
opposite selection rule: every arm that matches contributes its body's alternatives, instead of
the first one winning. `or` already changes meaning by position in toylang -- Bool disjunction
in an arm's guard, arm separator in a chain ([boolean operators](../docs/reference/operators/boolean.md)--
so a third positional reading is precedented rather than new machinery. Whether that third
reading can be told apart from the second is the question the candidates below have to answer.

## What the three surveyed traditions actually do

### Prolog: two precedents, clause-level and goal-level

Prolog has two different branch-wider mechanisms, and only one is an operator. Clause
selection is nondeterministic at the predicate level: every clause whose head unifies with the
call is tried, in source order, with the call site's continuation threaded between them. A
failed continuation backtracks to the next clause. No operator spells this; it is the meaning of
"a predicate". Cut (`!`) is what commits. Disjunction, `(Goal1 ; Goal2)`, is a separate
*goal-level* operator: succeed via Goal1's clauses or Goal2's, backtracking between them. Its
operands are goals, not terms -- `;` branches the continuation, it never builds a value of the
two sides. This is the important structural fact for toylang: clause-level backtracking (the
"every matching clause contributes" model) lives in Prolog's *clause* semantics, not in an
infix operator, while the explicit choice point is an infix goal combinator that Prolog spells
with `;` (ISO/IEC 13211-1; Clocksin and Mellish).

### Icon and Curry: infix expression combinators, no match surface

Icon has no pattern-match construct at all, and it does not need one: expressions are
*generators*, and `|` is an infix expression operator producing the ordered results of its left
side, then its right side ("goal-directed evaluation", resume-and-backtrack on failure).
Curry, the functional-logic descendant, spells the same choice `?` (`coin = 0 ? 1`) over
overlapping rule left-hand sides, with free variables and narrowing supplying the search. Both
traditions reach for an *infix expression combinator* when they want branch-wider -- never a
match-like chain. Icon's `|` is the closest precedent to the draft's `,` sketch, spelled
exactly the way the draft's `,` is meant to work: an ordered union of two generators'
outputs. (Griswold and Griswold, The Icon Programming Language; Hanus, Curry Report)..

### Haskell list monad: comprehensions for bind and guard, `mplus` for choice

The list monad makes search a *shape*, not an operator: a comprehension like
`[ (a,b) |a <- xs, b <- ys, b >= a*10 ]` desugars to
`xs >>= \a -> ys >>= \b -> guard (b >= a*10) >> return (a,b)`. Generators (`<-`),
filters, and yield are each dedicated comprehension syntax. Choice is a separate binary,
`MonadPlus`'s `mplus` (`<|>`) -- on lists, concatenation, again an infix expression
combinator, not a clause chain. toylang's `[ search ]` reifier ([draft.md](../draft.md#query-is-search))
is already comprehension-shaped: a pipe chain inside brackets collects the search's results
into a `Vec`. What is missing from that shape is exactly the choice-point junction, which
every surveyed tradition spells as an infix operator (Haskell Report, list comprehensions;
MonadPlus/`Alternative`).



### What the survey implies

- **Nobody spells branch-wider as a match chain.** When a language has both a match-like
  construct and branch-wider (Prolog), the match-like construct is clause-level backtracking
  (every matching clause contributes -- the complement of toylang's first-match-wins match),
  and the explicit operator (`;`) is a separate goal combinator. The two are different layers.


- **Branch-wider is always an infix expression combinator** (`;`, `|`, `?`, `<|>`), composing
  where expressions compose, and producing the ordered union of its sides' results. The draft's
  `,` sketch is exactly this shape; so does the "new infix operator" framing. The match-parallel
  framing is the one place toylang would depart from every surveyed precedent.



- **Order is load-bearing everywhere**: Prolog clause order, Icon `|` left-to-right, list
  monad `mplus` as concatenation -- all preserve source order, matching the draft's "fixed
  order" claim for nondeterminism. A match-parallel reading (`or`-arms, all contribute, in
  order) preserves order the same way, as long as the arms stay source-ordered. 

## Three candidates

Worked example, spelled under every candidate: *every label that applies*, the sharpest
contrast between first-match-wins and branch-wider. A committed match on `15` answers `"fizz"`
(only), because the first arm wins; the choice-point reading answers `["fizz","buzz"]`,
because both arms match. Pair-picking (the brief's suggested example, a cartesian product
with a constraint) turns out to need *no* choice point -- it is bind over xs, bind over ys,
prune with a guard, yield -- which is why a short pair-picking line accompanies each candidate
below only where the a-source itself branches. The surrounding pipeline is illustrative, as the
draft's is ([draft.md](../draft.md#query-is-search)).

### Candidate A: an `or`-joined match chain read as every-arm-contributes

The arm chain keeps its exact spelling, including `or`; the semantic flip is the whole change.

In a match, the first accepting arm wins. In a choice point, every accepting arm contributes
its body's alternatives -- zero alternatives for an arm whose guard is false. Binding comes
from the arm syntax itself (`Circle{r} -> r * r` already binds `r`), so no `as` machinery is
needed:


```toylang
fn labels(n: Int) -> Vec<Str> =
    [ n | . % 3 == 0 -> "fizz" or . % 5 == 0 -> "buzz" ]
```

`labels(15)` -> `["fizz","buzz"]`;`labels(3)` -> `["fizz"]`;`labels(7)` -> `[]`. The
`[ ]` reifier already marks this as a search -- a bare `[ expr ]` collects a search's results
into a `Vec` -- so the "generator reading" can be context-marked with no new token at all. If
context-marking is not wanted, the same spelling works with a distinct joining word reserved
for generator chains (e.g. `also`), trading one more positional meaning of `or` for an
unambiguous one. This is the one candidate that answers the maintainer's question literally:
"yes, spell it parallel to match, with the arm chain read as a generator."

Pair-picking with a branching a-source, the `or`-joined bind arms are the choice point:

```toylang
fn pairs(xs: Vec<Int>, zs: Vec<Int>, ys: Vec<Int>) -> Vec<{a: Int, b: Int}> =
    [
        xs | .{a: .}  or  zs | .{a: .}  ->  ys | .{b: .}  ->  .b >= .a * 10 -> {a: .a, b: .b}
    ]
```

The first two arms are alternative bindings of `a` (branch wider at one junction); the rest of
the chain binds `b` and prunes. Every arm that matches contributes, so the two a-sources
both survive into the cartesian step.

### Candidate B: `;`, Prolog goal disjunction, the draft's `,` re-tokened

Prolog's spelling, fordirectness: an infix operator whose operands are multi-valued
expressions, and whose result is the ordered union of their alternatives. This is the draft's
`,` sketch with a token that does not collide with the list/record separator:

```toylang
fn labels(n: Int) -> Vec<Str> =
    [ n | . % 3 == 0 -> "fizz" ; n | . % 5 == 0 -> "buzz" ]
```

Each side is itself an expression (here, a partial guard chain yielding `Opt<Str>`), and
`;` widens: present alternatives from both sides, left to right. Same answers as candidate
A `branch(a, b)` variant trades a new token for a named function, at the cost of not composing inline
where expressions compose.

### Candidate C: `//`, the retired arm separator resurrected

`//` is not available for this job without a fight. It was toylang's arm separator until
`or` replaced it ([match reference](../docs/reference/operators/match.md), "the pre-`or`
arm separator `//` is retired"), and the parser confirms it is no longer a token
([parse.rs](../src/parse.rs)).
Its heritage is jq's `//` -- but jq's `//` means *default-on-absent*, first non-empty wins, not
explore-every-alternative, so adopting it for branch-wider would either redefine it away from
the cited heritage or import the opposite of the desired semantics. And
[language-oddities.md](./language-oddities.md#-separates-match-arms) already records that
`//` reads as a comment to C-family readersand integer division to Python readers -- the same
collision that motivated retiring it. Spelled anyway, with the explore-all semantics made
explicit:

```toylang
fn labels(n: Int) -> Vec<Str> =
    [ n | . % 3 == 0 -> "fizz" // n | . % 5 == 0 -> "buzz" ]
```

Same answers as A and B, at the cost of resurrecting a retired, misread token.

## Assessment

| | new syntax | reuses match surface | collision / misread risk | survey precedent |
|---|---|---|---|---|
| A: `or` chain as generator | none (context-marked) |the whole arm chain, `or` included | `or` gains a third positional meaning | none -- the one place toylang would depart |
| B: `;` (or `branch(...)`) | one infix operator (or one name) | none | none (`;` unused;`branch` unused) | Prolog `;`, jq-adjacent |
| C: `//` | none (reuses a retired token) | none | comment/division misreads; collides with jq's real `//` semantics | jq `//`, redefined |

The axes that actually separate the candidates are not expressiveness -- all three renderthe
worked example identically -- but what a reader has to unlearn. A needs no new token and
reuses the most toylang-shaped surface, but overloads `or` further (or needs a marker word).
B is unambiguous and precedented, but adds an infix operator to a language whose branching is
otherwise match-shaped. C is the only one with a *wrong heritage*: the token's documented
meaning (jq first-non-empty, and the comment reading) points the other way, and the token was
already retired for exactly that reason.

## Recommendation

Answer the round's question with candidate A, context-marked: an `or`-joined arm chain inside
the existing `[ ]` reifier (or any context expecting a `Vec`)is a generator chain, every arm
that matches contributes, in source order; the same chain elsewhere stays a committed match.


That is "spelled parallel to match syntax" in the strongest sense -- the same surface syntax,
the semantic flip the question is about, and no new token. It is also the reading with the
least new machinery: the arm shape, the `or` join, and the `[ ]` reifier all already exist,
and `or` already shifts meaning by position. What the next round has to settle is the marker
sub-question: context (`[ ]`) versus a reserved joining word (`also`), and whether a guard arm that
is false contributes nothing (the candidate's zero-behavior) is the right partiality rule for a
generator chain. The surveyed precedent that supports A is Prolog's *clause-level*
backtracking --the every-matching-clause-contributes model is the complement of toylang's
first-match-wins match, and A is that model wearing toylang's own arm syntax

Sources:
- [Match](../docs/reference/operators/match.md) and [boolean operators](../docs/reference/operators/boolean.md)
  (toylang's `or` positions). 
- [Query is search](../draft.md#query-is-search) (the search vocabulary, the `,` sketch,
  the `[ ]` reifier). 
- [language-oddities.md](./language-oddities.md#-separates-match-arms) (the `//` misread
  collision, and its retirement).
- ISO/IEC 13211-1 (Prolog; Clocksin and Mellish, *Programming in Prolog* (`;` goal
  disjunction, clause backtracking, cut).
- Griswold and Griswold, *The Icon Programming Language* (`|` alternation, goal-directed
  evaluation; Hanus, *Curry: An Integrated Functional Logic Language* (ed.),`?` choice, free
  variables, narrowing).
- Haskell 2010 Report (list comprehensions, desugaring via `>>=` and `guard`; and the
  MonadPlus/`Alternative` class (`mplus`/`<|>`) as list choice). 

## Provenance

Human-authored: the maintainer's question framing and the three current sketches (`;`,
`branch(...)`, `//`)from the search-vocab-choice-cut-fold round, as recorded in board.yaml;
the draft's "Query is search" vocabulary and its `//`-retirement note. Derived: the Prolog,
Icon/Curry, and Haskell survey from the cited primary sources; toylang's `or` positions and the
`[ ]` reifier from the match/boolean references and draft. Agent-invented: the concrete
match-parallel spelling (candidate A: an `or`-chain read as every-arm-contributes), the
context-marking proposal, and the all-labels worked example.