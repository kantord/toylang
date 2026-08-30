# and, or, not

Three keywords over [Bool](../types/bool.md), spelled as words rather than symbols. `and` and
`or` are infix and yield a `Bool`; `not` is prefix.

```case
bool_connectives
```

## Precedence

Loosest to tightest: `or`, then `and`, then `not`, then everything a comparison is built from.
`not` sitting below comparison is what lets `not is_digit(c)` and `not c == boundary` both read
without parens; the alternative, binding it as tight as unary `-`, needs parens at every use.

```case
bool_precedence
```

Against the rest of the language, both connectives bind tighter than the
[conditional](conditional.md) and looser than everything in [arithmetic](arithmetic.md) and
[comparison](comparison.md). So `a == 1 or b == 2` is one disjunction of two comparisons, and
`x if a or b else y` needs no parens around the condition.

## The right side may not run

`and` evaluates its right side only when the left is true, `or` only when the left is false.
That is observable rather than an optimization, because [division](arithmetic.md) by zero and
`!` on an absent [Opt](../types/opt.md) both stop the program: a guard written to the left of
`and` really does protect what is to the right of it.

```case
bool_short_circuit
```

## `or` also separates match arms

`or` is the [match](match.md) chain's arm separator, and was that first. One spelling, two
readings, told apart by where the `or` is read:

- Inside an arm's left side -- a guard still being read -- it is disjunction, so
  `. == 4 or . == 7 -> "middle"` is one arm with a two-clause guard.
- After an arm's body, which is finished, it is the separator that starts the next arm.

```case
bool_or_splits_from_arms
```

The consequence is one rule to remember: a Bool `or` written directly in an arm's body needs
parens, since the bare spelling there is the separator. Nothing else does.

```case
bool_or_in_arm_body
```

Everywhere below a chain's top level -- inside parens, a call's argument, a condition between
`if` and `else` -- `or` is disjunction again, because none of those positions can be a chain's
top level.
