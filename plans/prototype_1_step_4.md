# Step 4: the filter

```
[1, 2, 3][] | select(. >= 2)
```

yields `[2, 3]`.

The first program that could not be written in a language without this design. It is also much
larger than the three steps before it, and if it stalls the natural split is to land projection
and composition first, then `select`.

## Adds

Int literals and a `Vec` literal. `[]` as projection. `|` as composition. `.` as the implicit
subject. `,`. Comparison yielding `Bool`. `select` as the one builtin, taking an unevaluated
filter as its argument.

## What this step is actually testing

Under C1, none of this leaves the value layer. `[1,2,3][]` is a projection of a `Vec` and yields
a `Vec` view; `select` over it is a mask and yields a `Vec`. So the program's type is `Vec<Int>`
with extent known to be at most three, and there is no effect multiplicity anywhere.

That is the one-way-shift proposal being taken at its word. Two things would falsify it here,
and both are worth watching for rather than working around:

- needing an effect annotation to type any of these expressions
- `select`'s result not being expressible as a `Vec` of unknown-but-bounded length

The load-bearing assumption is that `|` applied to a `Vec` is elementwise. That is what makes
`.name` work at step 5 without a `map`, and it is the same broadcast question as Q2, restricted
to the unary case where it is uncontroversial.

## `select` takes a filter, not a value

`select(. >= 2)` cannot evaluate its argument first, because `.` is bound per element. The
argument is an expression checked against the element type with `.` in scope. This is the jq
arrangement and it is cheap, but it means the checker needs a notion of "check this expression
later, in this subject context", which the three earlier steps did not.

## Negative cases

```
(1,2) + 3              # ERROR: `+` requires exactly one value on the left, found 2   (C2)
select(.)              # ERROR: select expects Bool, found Int
```

The first is the one that matters. It is Q2 deferred into a diagnostic, and when Q2 settles this
is the test that changes.
