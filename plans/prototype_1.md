# Prototype 1

One program, compiled and run end to end, with the type checker doing real work. Written from
scratch rather than forked from jaq: jq is a reference, not a conformance target, so inheriting a
front end built around jq's surface syntax buys a corpus we are no longer trying to pass.

Target program, reached at step 5:

```
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users[] | select(.age >= 18) | .name
```

## Stack

| Layer | Choice | Why |
|---|---|---|
| Lexer, parser | Hand-written, recursive descent with a Pratt loop | About fifteen productions. `\|` and `,` sit below comparison, which is unusual enough to want a precedence table we own. |
| Diagnostics | Byte spans in the AST from step 1; `ariadne` when a message needs to point at source | Spans are painful to retrofit, formatting is not |
| Checker | Hand-written bidirectional | The draft already specifies it: named functions annotate, lambdas are checked against an expected type |
| Backend | Emit Lua source, run through `mlua` with the vendored interpreter | On the roadmap already, hermetic in tests, and the emitted code is readable when something is wrong |
| Values | `serde_json` for stdin, from step 5 only | Steps 1 to 4 have no input |
| Tests | `insta` snapshots over `tests/cases/`, one file per program, plus negative cases asserting the compile error | |

Single crate. Split to a workspace when a second backend lands, not before.

## Two provisional commitments

Both are recorded here so that reversing them is a visible decision rather than a rediscovery.

**C1. Value layer only.** Under the one-way-shift proposal in `draft.md`, `[]` is projection and
stays in the value layer, and effect multiplicity is born only from streaming input. Prototype 1
has no streaming input, so it implements no effect layer at all and every expression has
statically known extent. Everything lowers to nested loops with an accumulator; no coroutines,
no CPS, and Q5 is not sidestepped so much as absent.

This makes the prototype an experiment on the proposal. If some construct here turns out to need
effect multiplicity without streaming input, that is evidence against the proposal and against
Q1's lean, and it is cheaper to find out at this size.

**C2. Binary operators require exactly one value on each side.** Q2 is open, so the undecided
case is a compile error rather than a silently chosen semantics:

```
(1,2) + 3     # ERROR: `+` requires exactly one value on the left, found 2
```

Same move the draft leans toward for Q3. When Q2 settles on broadcast or on explicit `cross` and
`zip`, this error is where the answer gets installed.

## Steps

Each is a commit, each leaves `cargo test` green.

1. [Walking skeleton](prototype_1_step_1.md) -- `"hello world"`
2. [Real parser](prototype_1_step_2.md) -- `"hello " + "world"`
3. [Functions](prototype_1_step_3.md) -- `fn greet(who: Str) -> Str`
4. [The filter](prototype_1_step_4.md) -- `[1,2,3][] | select(. >= 2)`
5. [Typed input](prototype_1_step_5.md) -- the target program

Step 4 is much larger than the others and is where the design first shows up. Steps 1 and 2 are
plumbing, and their plans are short because there is little to decide.

## Not in prototype 1

`[...]` and the effect layer (nothing here streams, per C1), `Opt`, `Json`, the error effect and
`?`, `..`, `//`, `=` and `|=`, lenses as first-class values, object construction, string
interpolation, `fold`, explicit `|x|` lambdas, recursion, user-defined named types, and the
native and JavaScript backends.

Object construction is the obvious first thing to add afterwards, because a map key requiring
exactly one value is the next hazard on the draft's list.
