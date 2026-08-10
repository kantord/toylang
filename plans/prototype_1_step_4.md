# Step 4: the filter

```
[1, 2, 3][] | select(. >= 2)
```

yields `[2, 3]`.

The first program that could not be written in a language without this design. It is also much
larger than the three steps before it, and if it stalls the natural split is to land projection
and composition first, then `select`.

## Adds

A `Vec` literal and `Vec<T>` in the type syntax. `[]` as projection. `|` as composition. `.` as
the implicit subject. Comparison yielding `Bool`. `select` as the one builtin, taking an
unevaluated filter as its argument. Int literals moved to step 3.

`,` is a separator inside a literal and nothing else. As an operator it would build a `Vec`,
which `[...]` already does.

## What this step is actually testing

Under C1, none of this leaves the value layer. `[1,2,3][]` is a projection of a `Vec` and yields
a `Vec` view; `select` over it is a mask and yields a `Vec`. So the program's type is `Vec<Int>`
with extent known to be at most three, and there is no effect multiplicity anywhere.

That is the one-way-shift proposal being taken at its word. Two things would falsify it here,
and both are worth watching for rather than working around:

- needing an effect annotation to type any of these expressions
- `select`'s result not being expressible as a `Vec` of unknown-but-bounded length

This was written expecting `|` applied to a `Vec` to be elementwise. It cannot be. If `|` hands
`select` one element, `select` must return zero-or-one, which is `Opt`, which is the effect-layer
machinery C1 says does not exist. So `|` is plain composition that rebinds `.`, and the operators
distribute over a `Vec` themselves. That is also what has to make `.name` work at step 5.

Neither falsifier fired, but three of jq's operators came out trivial. See
[the research note](../research-log/a-pure-value-layer-dissolves-jqs-iteration-operators.md).

## `select` takes a filter, not a value

`select(. >= 2)` cannot evaluate its argument first, because `.` is bound per element. The
argument is an expression checked against the element type with `.` in scope. This is the jq
arrangement and it is cheap, but it means the checker needs a notion of "check this expression
later, in this subject context", which the three earlier steps did not.

## Negative cases

```
[1, 2] + "a"           # ERROR: `+` does not apply to Vec<Int>              (C2)
[1, 2, 3] | select(.)  # ERROR: expected Bool, found Int
```

The first is the one that matters, and it did not come out as C2 predicted. C2 says binary
operators require exactly one value per side, which presumes cardinality is tracked apart from
the type. Under C1 it is not: a `Vec` is a type, and "no operator over a `Vec`" is ordinary
typing. C2 has no separate content in prototype 1 and gets it back only alongside the effect
layer. When Q2 settles on broadcast or on explicit `cross` and `zip`, this is the test that
changes.
