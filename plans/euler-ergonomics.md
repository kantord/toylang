# What the Euler corpus asks for: an ergonomics review

Commissioned by [kantord/toylang#104](https://github.com/kantord/toylang/issues/104): read
every solution under [docs/examples/euler/](../docs/examples/euler/00-spoiler-warning.md),
compare with how the same problems read in other languages, and propose ways to make the
language more elegant and ergonomic while keeping it minimal. Reviewed at the commit this
file lands on: 21 solved pages, 9 skip pages, cross-checked against docs/reference so
nothing below proposes what already exists. Claims about current behavior come from the
reference or from programs run against `toylang run` during this review; the two
load-bearing runs are quoted in full.

The comparison points are jq (the nearest ancestor, and the source of `select`/`map`/the
pipe), Python (the conditional's ancestor and the most common Euler idiom), and Haskell
(the recursion-first reference: what these programs look like in a language built for
exactly this shape).

One timing note: [sort and reverse](../docs/reference/builtins/sort.md) landed
([#86](https://github.com/kantord/toylang/issues/86)) hours after the batch-3 pages were
written, so [problem 22](../docs/examples/euler/22-names-scores.md),
[23](../docs/examples/euler/23-non-abundant-sums.md), and
[29](../docs/examples/euler/29-distinct-powers.md) still cite the missing `Vec` sort as
current. Those citations are stale; this review treats sort as existing.

## What already reads well

The comparison is not one-sided, and the parts that hold up should not be churned in the
name of ergonomics.

The closed-form pages ([1](../docs/examples/euler/01-multiples-of-3-and-5.md),
[6](../docs/examples/euler/06-sum-square-difference.md),
[28](../docs/examples/euler/28-number-spiral-diagonals.md)) are as short as any language's
version. Pipe chains read exactly like the jq they descend from --
[problem 9](../docs/examples/euler/09-special-pythagorean-triplet.md)'s
`range(1000) | select(. > a) | select(. < 1000 - a) | ...` needs no apology. And the
record-argument call convention is a quiet win at real call sites:
[problem 11](../docs/examples/euler/11-largest-product-in-a-grid.md)'s
`direction({g: g, dr: 1, dc: -1, rmax: ..., cmin: 3, cmax: ...})` names all six things it
passes, where the Python equivalent is six positional values the reader must count. The
costs of the record convention are real but they sit elsewhere (the body-side `p.` prefix;
see [parameter destructuring](#record-parameters-could-destructure-the-way-match-arms-already-do)).

The skip pages also earn their place: each names its blocker and links the issue, which is
what made this review mechanical to do.

## Suggestions

Ranked by pain erased per concept added. Each is sized to be its own decide or build row;
a [summary list](#proposed-rows) is at the end.

### `sum` and `max` as builtin reductions

The single largest cost in the corpus. Twenty of the helper functions across the 21 solved
pages exist only to re-implement summation, maximum, or product by hand:

- maximum: `max2` and `max_vec` ([4](../docs/examples/euler/04-largest-palindrome-product.md)),
  `max2` again at `Int64` width and `best`
  ([8](../docs/examples/euler/08-largest-product-in-a-series.md)),
  `maximum_of` and `maximum` ([11](../docs/examples/euler/11-largest-product-in-a-grid.md)),
  `best_of` and `find_best` ([26](../docs/examples/euler/26-reciprocal-cycles.md))
- summation: `even_fib_sum`'s accumulator
  ([2](../docs/examples/euler/02-even-fibonacci-sum.md)), `inner_sum` and `outer_sum`
  ([17](../docs/examples/euler/17-number-letter-counts.md)), `col_sum`
  ([13](../docs/examples/euler/13-large-sum.md)), `sum_range`
  ([21](../docs/examples/euler/21-amicable-numbers.md)), `sum_rings`
  ([28](../docs/examples/euler/28-number-spiral-diagonals.md)), `sum_ints`
  ([29](../docs/examples/euler/29-distinct-powers.md)), `sum_vec` and `total`
  ([30](../docs/examples/euler/30-digit-fifth-powers.md))
- product: `window` ([8](../docs/examples/euler/08-largest-product-in-a-series.md)),
  `factorial` ([24](../docs/examples/euler/24-lexicographic-permutations.md)), `ipow`
  ([29](../docs/examples/euler/29-distinct-powers.md))

Every comparison language ships the reduction: Python `sum`/`max`, Haskell
`sum`/`maximum`, jq `add`. Problem 1 in each is one line --
`sum(x for x in range(1000) if x % 3 == 0 or x % 5 == 0)`,
`[range(1000) | select(.%3 == 0 or .%5 == 0)] | add` -- and the toylang page dodged the
comparison only by finding a closed form. Problem 4 is the sharpest exhibit. Haskell:

```haskell
maximum [a*b | a <- [100..999], b <- [a..999],
               let s = show (a*b), s == reverse s]
```

The toylang page is thirty lines, and the interesting ones (generate products, keep
palindromes) are already as good: `range(1000) | select(. >= a) | map(a * .) |
select(. == reverse_num({n: ., acc: 0}))`. Everything else on the page -- `max2`,
`max_vec`, the sentinel `0` padding, the nested search -- is the missing `max`, plus the
[stack-depth engineering](#self-tail-calls-become-loops-on-every-backend) that hand-rolled
folds drag in. With a `max` builtin the page collapses to `reverse_num`, the row
expression, and one outer call, and its entire explanatory preamble about recursion depth
evaporates.

The shape that fits what exists: `sum(v)` over `Vec<Int>` and `Vec<Int64>` (empty sums to
0), `max(v)` over the same element types [sort](../docs/reference/builtins/sort.md)
accepts, returning `Opt<T>` because the empty `Vec` has no maximum -- the same answer
indexing already gives to absence, and more honest than Python's `max([])` exception or
jq's `add` returning `null` untyped. Implemented as builtins, each backend emits a loop,
so the reduction costs no stack depth anywhere.

Two boundaries worth drawing in the decide session:

- The corpus never once wants `min` -- every extremum in thirty problems is a maximum.
  Adding it costs little, but it would be built for symmetry, not for a caller
  ([AGENTS.md](../AGENTS.md)'s line), so let the decision be conscious. Same for
  `product`: three pages want one, and two of those (`factorial`, `ipow`) are index-driven
  rather than Vec-shaped and would not use it.
- This is deliberately not the general `fold`. [draft.md's single-pass
  composition](../draft.md#single-pass-composition) and the [parallel
  basis](../draft.md#the-primitive-set-cannot-be-fold-and-recursion) already lay out the
  real design: `reduce` over an associative operator is basis-primitive, general `fold` is
  a sequential leaf. `sum` and `max` are the two associative instances the corpus begs
  for, they commit to nothing about that larger surface, and they can land years before it
  does. The decide session should confirm the names don't collide with the fold-block
  design rather than re-litigate it.

Prelude-vs-builtin interacts with [#105](https://github.com/kantord/toylang/issues/105)
(what could move to the prelude): a prelude `sum` written as toylang source today would be
a linear recursion and inherit the very stack ceiling these pages engineer around, so
prelude placement is only honest after [tail calls become
loops](#self-tail-calls-become-loops-on-every-backend).

### Self-tail-calls become loops, on every backend

Recursion is the language's only loop, and the corpus shows users paying for that in a
currency the language never defined: backend call-stack budgets. Six pages replace the
natural linear fold with a halving recursion chosen for depth, not speed
([4](../docs/examples/euler/04-largest-palindrome-product.md),
[8](../docs/examples/euler/08-largest-product-in-a-series.md),
[21](../docs/examples/euler/21-amicable-numbers.md),
[26](../docs/examples/euler/26-reciprocal-cycles.md),
[28](../docs/examples/euler/28-number-spiral-diagonals.md),
[30](../docs/examples/euler/30-digit-fifth-powers.md)). Three more chunk a walk into
nested runs purely to cap frames: [17](../docs/examples/euler/17-number-letter-counts.md)
sums 1000 numbers as ten runs of a hundred, [19](../docs/examples/euler/19-counting-sundays.md)
walks 1212 months as years-of-months, [26](../docs/examples/euler/26-reciprocal-cycles.md)
walks a division cycle a hundred digits per call frame.
[Problem 22](../docs/examples/euler/22-names-scores.md) was skipped over exactly this: "every
shape a hand-written one takes recurses once per element ... five times the ceiling."
Each of these pages spends a paragraph explaining its own contortion; the reader pays
alongside the author.

The distortion is an implementation detail leaking. The fix is a contract, in the spirit
of [Int is 32 bits and wraps](../docs/adr/0006-int-is-32-bits-and-wraps.md): pick the
observable behavior every backend can carry exactly, write it down as law. Here that
contract is *a self-tail-call runs in constant stack*. Every backend has a constant-stack
form to lower into: Lua guarantees proper tail calls in the language itself, jq has
`until`/`reduce` (and TCO of its own for tail-recursive filters), and the other five
emitters own their codegen and can rewrite a self-tail-recursive function into the loop
it denotes. Restricting the promise to self-calls (not mutual recursion) keeps the jq
backend inside what it can honestly do -- its forward-reference limits already make
cross-function cycles second-class there, per
[functions](../docs/reference/syntax/functions.md).

Most of the corpus's accumulator walks are already in tail form because their authors were
writing loops by hand: `cd_loop` (12), `run_months`/`run_years` (19), `cycle_inner` (26),
`month_advance`'s driver, `largest` (3), `has_divisor` (7). Under the contract those simply
become safe at any length, the chunked pages flatten, problem 22's blocker disappears, and
the halving pattern goes back to being a speed choice instead of a survival one. Programs
not in tail form (`digit_power_sum`, `nth_perm`, `lcm_upto`) change nothing and lose
nothing.

This is the one suggestion that adds zero surface: no syntax, no builtin, no type. It
retires a whole genre of explanation from the docs.

### Local bindings: evidence for the open question

[#87](https://github.com/kantord/toylang/issues/87) already tracks the where/as-shaped
gap and asks for the shape to be grilled. The corpus supplies the evidence, and this
review adds two verified observations about what exists today.

First, the pipe is already a one-shot `let`. This runs today (verified during this
review):

```text
fn f(p: {m: Int, r: Int}) -> Int =
    cycle({a: p.m + 1, b: p.m * 2}) |
        (.a + p.r if .b > 10 else .a - p.r)
```

The right side of a pipe is any expression, `.` is the bound value, and the enclosing
parameter stays in scope. None of the Euler pages use this, which is itself a finding:
the idiom is legal but nothing teaches it, and
[the pipe's reference page](../docs/reference/operators/pipe.md) frames the right side as
a transformation stage rather than as a binding site.

Second, the reason it cannot carry the load `let` carries elsewhere: `.` rebinds at every
`map`/`select` body, so a piped-in binding is unreachable exactly where programs need it
most (also verified: `[10, 20] | (range(2) | map(. + .))` yields `[0,2]` -- inside the
map, the outer subject is gone). One anonymous slot, shadowed at every inner boundary,
is the whole current budget.

What that budget costs, in the corpus:
[problem 26](../docs/examples/euler/26-reciprocal-cycles.md)'s `cycle_outer` spells
`cycle_inner({m: p.m, r: p.r, count: p.count, steps_left: 100})` four times -- and
because nothing memoizes, the hundred-step walk actually runs up to three times per
chunk, a real constant-factor cost on all seven backends, not just noise on the page.
[Problem 13](../docs/examples/euler/13-large-sum.md) evaluates `column_total(...)` twice
per column (once for the carry, once for the digit);
[problem 24](../docs/examples/euler/24-lexicographic-permutations.md) computes
`factorial(extent(p.remaining) - 1)` twice per level;
[problem 29](../docs/examples/euler/29-distinct-powers.md)'s `best_mult` calls
`find_root_for_mult` twice per candidate.

So the corpus's requirements for whatever spelling #87 picks: more than one binding live
at once, and bindings that survive into `map`/`select` bodies -- the two things the
anonymous `.` cannot do. jq's `expr as $x | ...` is the existence proof that a
pipe-integrated named form composes with `.`-rebinding; Haskell's `let`/`where` is the
expression-shaped alternative.

### Booleans: what lands when #96 does

[#96](https://github.com/kantord/toylang/issues/96) (ratified: `and`, `or`, `not` as
keywords) is in flight, so this is a sweep list rather than a suggestion. The corpus
idioms it retires, for whoever does the sweep:

- `1 == 0` and `1 == 1` as constants:
  [21](../docs/examples/euler/21-amicable-numbers.md)'s `is_amicable` ends in `1 == 0`;
  [29](../docs/examples/euler/29-distinct-powers.md) filters with
  `select(is_dup(...) != (1 == 1))` -- negation spelled as inequality-with-true.
- Conjunction as stacked selects:
  [9](../docs/examples/euler/09-special-pythagorean-triplet.md)'s
  `select(. > a) | select(. < 1000 - a)`,
  [29](../docs/examples/euler/29-distinct-powers.md)'s
  `select(. * . <= top) | select(is_primitive(.))` -- each becomes one `select` with
  `and`.
- Disjunction as a conditional: problem 1's filter, spelled directly, is
  `select(1 == 1 if . % 3 == 0 else . % 5 == 0)` (verified working during this review);
  the page avoided it with a closed form.

Note [#96 already plans](https://github.com/kantord/toylang/issues/96) the `not(x)` sweep
for the char-class docs; the Euler pages above belong on the same pass. `true`/`false`
literals are not in #96's text -- [Bool](../docs/reference/types/bool.md) still says
"nothing shorter exists yet" -- and the corpus wants them exactly twice, so they can ride
along or wait.

### `first`: letting a search stop

[Problem 7](../docs/examples/euler/07-10001st-prime.md) tests every candidate up to
`range(104744)` and indexes `[10000]!`. The answer is 104743: the bound is the answer
plus one. [Problem 12](../docs/examples/euler/12-highly-divisible-triangular-number.md)
does the same with `range(12376)` and the winning index is 12375 -- again the answer plus
one. Both constants can only be written down after solving the problem somewhere else,
which the problem-7 page admits ("a fixed upper bound known to hold the
ten-thousand-first prime"). Both pages then pay for the honesty at runtime: jq spends
seconds scanning candidates past the point a lazy search would have stopped, the same
cost class [#90](https://github.com/kantord/toylang/issues/90) and
[#93](https://github.com/kantord/toylang/issues/93) track for the skipped pages.

jq itself does not have this problem: its generators are lazy, so the tenth-thousand-first
prime is `nth(10000; primes)` over an unbounded `range(2; infinite)` and evaluation stops
at the hit. Haskell's `primes !! 10000` is the same shape. toylang's `range` builds a real
`Vec` and its `Stream` is born only at stdin, so there is no spelling in the language
today that stops early.

[draft.md's search table](../draft.md#query-is-search) already names `first(f)` as cut,
and the [admissibility discussion](../draft.md#the-primitive-set-cannot-be-fold-and-recursion)
covers `any`/`all` short-circuiting. So the decide question is not whether the construct
belongs -- the draft says yes -- but how much of it to take early and whether it can be
value-layer only: `first` over a `Vec` pipeline whose short-circuit is observable purely
as speed would fit the language's stance that vectorizability stays silent
([Q8](questions.md#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization)),
and it removes the magic-bound genre without touching the Stream rules.

### Slices as index specs

Three pages rebuild by hand what an index range would say directly.
[Problem 13](../docs/examples/euler/13-large-sum.md) takes the first ten entries as
`fn first_ten(v: Vec<Int>) -> Vec<Int> = range(10) | map(v[.]!)`.
[Problem 24](../docs/examples/euler/24-lexicographic-permutations.md)'s `remove_at`
rebuilds everything after the dropped index one cons at a time.
[Problem 8](../docs/examples/euler/08-largest-product-in-a-series.md)'s thirteen-entry
window walks indices recursively.

The [index-spec algebra](../docs/reference/operators/specs.md) already has the concept
slot: keep (`[]`), narrow (`select`), collapse (an index). A slice `v[lo:hi]` is narrowing
by position, jq's own `.[2:5]`, and it composes with everything the other specs compose
with. With [#97](https://github.com/kantord/toylang/issues/97)'s ratified `Vec` `+`,
`remove_at` becomes `v[0:i] + v[i+1:]` and the cons idiom `concat([[x], acc])`
(problems 13, 24, 29) becomes `[x] + acc`.

The decide question is the boundary behavior: jq clamps out-of-range slices to the valid
range; toylang's collapsing index answers absence with `Opt`. A slice that clamps is the
useful default (every corpus use site wants the prefix that exists), but it should be a
conscious choice against the `Opt` story, not an inheritance.

### `sort_by`, `max_by`: comparing through a projection

[Problem 26](../docs/examples/euler/26-reciprocal-cycles.md) carries `best_of`, a
hand-written two-record comparator, because its maximum is over `{d, len}` pairs compared
by `.len`. [Problem 22](../docs/examples/euler/22-names-scores.md), half-unblocked now
that sort exists, needs rank-after-sorting next. [sort](../docs/reference/builtins/sort.md)
is deliberately restricted to natively-ordered element types; `sort_by(.key)` and
`max_by(.key)` order records through a scalar projection, which every backend can do with
a key comparator, no general record ordering required. jq has exactly this pair for
exactly this reason. Depends on `max` existing first; the projection-body machinery
(`map(.name)`) already exists to hang it on.

### Record parameters could destructure the way match arms already do

Every multi-argument function in the corpus takes `p: {...}` and pays the `p.` prefix on
every mention: `gcd({a: p.b, b: p.a % p.b})` where Python writes `gcd(b, a % b)`.
[Problem 19](../docs/examples/euler/19-counting-sundays.md)'s `month_advance` mentions
`s.` eleven times in four lines. The call-site half of the convention is worth keeping
(see [what already reads well](#what-already-reads-well)); the body-side tax is not
load-bearing.

The language already binds record fields fresh in one place:
[match arms](../docs/reference/operators/match.md), where `circle{r} -> r * r` names the
payload's fields directly. Letting a function parameter use the same pattern -- some
spelling of `fn gcd({a, b}: {a: Int, b: Int}) -> Int = a if b == 0 else gcd({a: b, b: a % b})`
-- removes the prefix without touching the call convention or adding a second parameter
list. Smallest win per page of anything here, but it touches every page, and the pattern
machinery is already in the checker. The decide question is the spelling and whether the
type annotation stays fully explicit (it should; signatures declare everything today).

### Big integers, re-tracked

[Problems 16, 20, and 25](../docs/examples/euler/16-power-digit-sum.md) are blocked
solely on arbitrary-precision integers, and
[problem 13](../docs/examples/euler/13-large-sum.md) exists only as a digits-in-a-Vec
workaround -- column arithmetic by hand, charming as a demonstration, forced as a
default. [#38](https://github.com/kantord/toylang/issues/38) described a two-step plan
(Int64, then BigInt), but it was closed when [Int64
landed](https://github.com/kantord/toylang/issues/83) and nothing now tracks the second
step; the [spoiler-warning page](../docs/examples/euler/00-spoiler-warning.md) still
calls it "the half of #38 that stays open," which no longer names an open issue.

This is not a near-term build: a third integer type has real minimalism costs (the
`Int`/`Int64` bridge discipline already shows the price of two), and jq's
doubles-envelope problem gets strictly worse. The suggestion is only a tracking row, so
the blocked pages point at something that exists.

## Proposed rows

In rank order; kind is a suggestion, the board decides.

1. `sum`/`max` reductions -- decide (shape, element types, `Opt` on empty, what stays
   out), then build. Erases the most code from the most pages.
2. Self-tail-call contract -- decide (contract or courtesy, self-only scope), then
   build per backend. Zero new surface.
3. Local bindings -- already [#87](https://github.com/kantord/toylang/issues/87)
   (decide); this file's [evidence](#local-bindings-evidence-for-the-open-question)
   feeds the grilling.
4. Boolean sweep of the Euler pages -- rides
   [#96](https://github.com/kantord/toylang/issues/96)'s build, no new row needed
   beyond a sweep note.
5. `first` -- decide (value-layer cut vs the draft's full search story).
6. Slices -- decide (clamp vs `Opt`), then build; pairs with
   [#97](https://github.com/kantord/toylang/issues/97).
7. `sort_by`/`max_by` -- build, after row 1.
8. Parameter destructuring -- decide (spelling), then build.
9. BigInt tracking issue -- file it, schedule nothing.
