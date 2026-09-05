# stdout/stderr effect model: what each round-1 option actually looks like

Research spike for [Q35](questions.md#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return),
requested after round 1 of the `stdout-stderr-effect-model` grill split the maintainer between
"a real type" (option C, Effect/IO) and noting the program already has Stream-typed inputs and
outputs (option B, extend the decided [Stream is the effect
layer](../draft.md#decided-stream-is-the-effect-layer-typed) model), while flagging stdin/stdout
splitting as a second open question riding along. This writes up what each of round 1's three
named options (opaque plumbing, Stream-carries-writes, real Effect/IO type) would concretely mean,
against what the language actually has today, verified by running real programs rather than
reasoning from the docs alone.

## What the language actually does today

One fact turned out to matter more than the `Sink` type itself: **a toylang program is exactly
one expression.** There is no sequencing construct in the grammar at all -- no `;`, no block, no
`Kind::Seq` in the AST. Two `jsonlines` calls back to back is not a Sink-specific restriction, it
is a parse error before the checker ever sees a type:

```toylang
jsonlines([1,2,3])
jsonlines([4,5,6])
```

```
expected end of program, found `jsonlines`
```

Within that one expression, output happens one of two ways. Default: the expression's value is
rendered by the type-driven printer once evaluation finishes. Exception: if the expression's type
is `Sink` -- reachable only through the `jsonlines` builtin, or a user function whose declared
return type is `Sink` and whose body is (transitively) a `jsonlines` call -- output happens by
writing as evaluation proceeds instead, and there is no result value at all:

```toylang
fn out(v: Vec<Int>) -> Sink = jsonlines(v)
out([1,2,3])
```

```output
1
2
3
```

`Sink` already carries every rule this research needs a name for: second-class (never in a
`Vec`, a record, or another `Stream`), legal only as the program's outermost expression or a
`Sink`-returning function's body, one instance today. Streaming through it works exactly the way
the effect-layer decision describes -- read one, transform one, write one, no buffering the whole
input:

```toylang
jsonlines(lines | map(length(chars(.))))
```

run against `ab\ncde\n`, prints `2` then `3` as each line is read, not both at once after
`lines` closes (`tests/streaming.rs` is what actually times this; the run above only confirms the
values).

stderr has none of this. Every backend refuses in its own words today (an unwrap on `none`, a
type error, a runtime panic), and the corpus harness deliberately checks only *that* they refuse,
never *what* they print. That is the compiler's or the runtime's own error reporting -- outside
program semantics entirely, not a second `Sink` a program can address.

So the real shape of Q35, sharpened: **not just "is there a stderr type," but "can a program
write more than once, to more than one place, in the same run at all."** None of the three
options below get to skip that question; they just answer it differently.

## Option A: opaque plumbing

Keep `Sink` exactly as restrictive as it is -- one per program, outermost position only, no
sequencing added -- and give stderr its own single-instance builtin alongside `jsonlines`,
symmetric in every rule:

```toylang
fn warn(msg: Str) -> Sink = eprintln(msg)
warn("could not parse line 3")
```

A program still picks exactly one of: return a value (default printer), write to stdout
(`jsonlines`), or write to stderr (`eprintln`) -- never a combination. This is "opaque" in the
literal sense: which byte stream a value lands on is a property of which single terminal node the
program happens to be, invisible to and unreachable from anywhere else in the expression.

This does not reach the case that motivated the question. `shell-out-build`'s `pipe_through`
needs to relay a subprocess's stdout *and* forward its stderr, in the same run, as the subprocess
produces both -- exactly the two-destinations-at-once shape this option structurally forbids. Any
fix that lets a program write to both needs sequencing, at which point the option has quietly
become option B in a smaller box. Opaque plumbing is a real, buildable answer to "can a program
address stderr as a type at all" (yes, trivially, the same way `jsonlines` already answers it for
stdout) and a non-answer to "can a program relay two streams from one subprocess," which is the
actual pressure behind the question.

## Option B: Stream-carries-writes (extend the decided Stream model)

Generalize `Sink` the way `Stream<T>` already generalized `Lines`: parameterize it (`Sink<T>`,
one instance of which -- `jsonlines`'s current job -- becomes `stdout: Vec<T> | Stream<T> ->
Sink<T>`), add a second instance addressing the other descriptor (`stderr: Vec<T> | Stream<T> ->
Sink<T>`), and add exactly one new thing neither `Stream` nor today's `Sink` has: a sequencing
form that combines two `Sink`-typed expressions into one `Sink`-typed program body. Everything
else -- second-class, born only at these two call sites, consumed/run exactly once, legal only
in outermost position -- carries over unchanged, because a sink is the write-side mirror of a
stream and the effect-layer decision already settled every one of those rules for the read side.

What the sequencing form buys, worked through the actual motivating case. Say `pipe_through`
produces two streams from one subprocess, `Stream<Str>` for its stdout and `Stream<Str>` for its
stderr (both already expressible: `Stream<T>` is a real type today). Relaying both, as they
arrive, would read:

```toylang
fn relay(out: Stream<Str>, err: Stream<Str>) -> Sink =
    stdout(out) or stderr(err)
```

`or` here is not new syntax invented for this: the language already reads `or` as two different
operators depending on position -- Bool disjunction, or the [match](../docs/reference/operators/match.md)
chain's arm separator, told apart by where it sits
([the boolean operators page](../docs/reference/operators/boolean.md)) -- so a third reading,
`Sink or Sink -> Sink` in sink position, extends a keyword the grammar already overloads rather
than introducing a new one. `and` is plain infix `Bool and Bool -> Bool` today, with no second
reading, so it is the wrong keyword to reach for here even though it reads more naturally as
"write this and write that." (Which keyword sequencing actually gets, `or` reused or something
new, is a real open call this spike is not making -- see "What this spike does not settle"
below.) `stdout(out) or stderr(err)` is not fully verifiable against the compiler as it stands
today, since `stdout`/`stderr`/sink-sequencing do not exist yet; it is the shape option B commits
to, in the same syntax family `jsonlines` already uses, not a design from a clean sheet.

This is the option the maintainer's own round-1 answer leaned toward ("isn't it already the same
types that we are using for our own program's own inputs and outputs?") and it is the smallest
addition of the three: one new type parameter on an existing type, one new builtin alongside an
existing one, one new sequencing form that has a real precedent for how `or` already reads
differently by position in this grammar.

## Option C: a real Effect/IO type

Introduce a genuinely new type -- `IO<T>` or `Effect<T>` -- distinct from `Stream` and `Sink`,
that a program returns instead of a plain value. `lines` and `inputs` stop being sources consumed
implicitly by the effect layer and become `IO<Stream<Str>>`-producing actions; `stdout`/`stderr`
become `Str -> IO<Unit>` (or `T -> IO<Unit>`) functions; a bind-like combinator (`>>=`, `then`, or
similar) sequences actions, the way Haskell's `IO` monad does. The top level runs whichever `IO`
action the program's body evaluates to, replacing "the body's value is printed" with "the body's
action is performed."

This is the heaviest of the three, for two reasons that are not about how much new syntax it
needs but about what it invalidates. First, every source and sink the language has today --
`lines`, `inputs`, `range`, `jsonlines`, and everything option B would add -- would need
rewrapping in `IO`, or the language would carry two competing effect notations side by side
(`Stream`/`Sink` for the cases that exist, `IO` for the new ones), which is exactly the kind of
seam this design has otherwise avoided. Second, it needs a type nothing here has yet: `Unit`, the
"no value" result of an action that only writes. `Sink`'s "no result type at all" is close but
not the same thing -- `Sink` is a special position a type checks into, not a type an ordinary
value can hold, which is what `IO<Unit>` would need to be composable with `>>=`/`then` the way an
`IO<Int>` is.

Option C answers "does a program write or return" the most explicitly of the three -- it makes
writing a value, sequenced like any other -- but it is a rewrite of the effect layer, not an
extension of it, and the language does not have a forcing case yet (a program that genuinely
needs to compose IO actions the way monadic code does, rather than just relay two streams) the
way `jsonlines(f(inputs))` was the forcing case for `Stream` itself.

## What this spike does not settle

- **Sequencing's spelling.** `or` above is one candidate, reused for its existing positional-
  overload precedent; a dedicated sequencing operator (closer to Rust's `;` or a real `then`) is
  the other real candidate, and choosing between them is a round-2 question, not something this
  spike can settle by writing more code.
- **Whether stdin/stdout splitting is a separate question or the same one.** The maintainer's
  round-1 note flagged this riding along; option B's `Stream<T>` sources already read stdin as
  one thing today (`lines`/`inputs`), so splitting it into "the part before a subprocess boundary"
  and "the part after" is a `pipe_through`-shaped question about sources, symmetric to sinks
  rather than a third thing this spike needs its own option for.
- **Whether a `Vec<T>` argument to `stdout`/`stderr` (as sketched in option B) should exist at
  all**, given `jsonlines` already accepts one -- kept for symmetry with the existing builtin, not
  re-litigated here.

## Recommendation for round 2

Present option B as the leading option, since it is the smallest true extension of a rule the
language has already committed to (`Stream`'s effect-layer typing) and it is what the
maintainer's own round-1 free text pointed at unprompted. Present option A alongside it as the
cheap, honest alternative that stays real but stops working the moment a program needs to relay
two streams from one subprocess -- worth naming explicitly, since it is the option that looks
like it should be enough until `pipe_through` is the thing being built. Present option C as the
correct answer if the language ever needs to compose IO actions generally rather than relay a
fixed pair of streams, and name what would force it (a real program needing that composition,
the same way the fused `jsonlines(f(inputs))` loop forced `Stream` itself) rather than building it
speculatively now.

Derived: the `Stream`/`Sink` rules from [the effect-layer decision](../draft.md#decided-stream-is-the-effect-layer-typed)
and `docs/reference/builtins/jsonlines.md`; `or`'s positional-overload precedent from
`docs/reference/operators/boolean.md` and `docs/reference/operators/match.md`. Agent-invented:
the three worked-out option shapes
themselves (the `Sink<T>`/`stdout`/`stderr`/sequencing sketch in option B most of all), since
round 1 named the three options but did not spell out what any of them would look like in code.
