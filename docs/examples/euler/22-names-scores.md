# Scoring a sorted list of names

Solves [Project Euler 22](https://projecteuler.net/problem=22). See the
[spoiler warning](00-spoiler-warning.md).

The five thousand names are problem-given data
([kantord/toylang#39](https://github.com/kantord/toylang/issues/39)); the fragment below scores
a synthetic list of three, and the real-sized check lives in `tests/euler_real_data.rs`, opt-in
via `just euler-data DIR`, which reads your own copy of the names file into the same program
and fails loudly on a wrong answer.

[`sort`](../../reference/builtins/sort.md) orders a `Vec<Str>` by codepoint, which on
upper-case ASCII names is alphabetical order, and `ranked_total` weights each name's score by
its one-based position in that order. A letter's value is its position in the alphabet, and a
[`Char`](../../reference/types/char.md) compares but does not convert to `Int`, so
`letter_value` counts instead: the letters of the alphabet that are not after `c` are exactly
`c`'s position. `names_total` exists to pin the type: `sort` is generic, so `parse(stdin)` fed
to it directly has no type to be read as. In the example COLIN, the name the problem statement
itself scores, is worth 53 and comes third of three, contributing 159 of the 227.

```toylang
fn letter_value(c: Char) -> Int =
  length(chars "ABCDEFGHIJKLMNOPQRSTUVWXYZ" | select(. <= c))


fn name_score(name: Str) -> Int =
  sum(chars name | map letter_value(.))


fn ranked_total(ordered: Vec<Str>) -> Int =
  sum(
    collect range(length ordered)
    | map((. + 1) * name_score ordered[.]!)
  )


fn names_total(names: Vec<Str>) -> Int = ranked_total(sort names)


names_total(parse stdin)
```

```input
["COLIN","ANNA","BOB"]
```

```output
227
```
