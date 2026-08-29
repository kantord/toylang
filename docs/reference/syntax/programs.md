# A program

A program is declarations followed by one expression, and the expression's value is what
prints. There are no statements, no `main`, and no print call: output needs no side effect,
because the program is the expression.

```toylang
# Comments start with `#` and run to the end of the line.
type Pair = {a: Int, b: Int}

fn total(p: Pair) -> Int = p.a + p.b

{sum: total({a: 1, b: 2})}
```

```output
{"sum":3}
```

The declarations are [type aliases](../types/alias.md), [enums](../types/enum.md), and
[functions](functions.md), in any order; a function may call one defined later. `pub` on a
`fn` marks it for a future module story and currently changes nothing in a program's own
file.

How the value prints: a top-level `Str` prints raw; everything else prints as compact JSON,
with record fields in the order their type declares them. A program whose outermost
expression is the
[`jsonlines`](../builtins/jsonlines.md) sink prints line by line instead and has no value.

Name casing is a rule, not a convention: a capitalized name is a type, a lowercase name is
a value. Field and variant names come from data and are exempt.

Everything is checked before anything runs: types, match coverage, stream linearity, the
32-bit literal rule. What remains at runtime is the arithmetic refusals (`/ 0`, `% 0`),
absent-value unwraps, and input that misses its declared type.
