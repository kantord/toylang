# Scoring a sorted list of names (skipped)

Skipped. [Project Euler 22](https://projecteuler.net/problem=22) scores a text file of over
five thousand names by alphabetical rank. The names are problem-given data with nowhere to
live in this repo, the blocker that keeps [problem 13](13-large-sum.md) and its neighbours on
synthetic input with the real-sized check opt-in
([kantord/toylang#129](https://github.com/kantord/toylang/issues/129)); the same protocol
could carry this page, but nobody has written it. The rank is no longer a blocker:
[`sort`](../../reference/builtins/sort.md) orders a `Vec<Str>` by codepoint, which on
upper-case ASCII names is alphabetical order. A letter's value would take some care, since a
[`Char`](../../reference/types/char.md) compares but does not convert to `Int`. See the
[spoiler warning](00-spoiler-warning.md).
