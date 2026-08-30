# Scoring a sorted list of names (skipped)

Skipped. [Project Euler 22](https://projecteuler.net/problem=22) scores a text file of over
five thousand names by alphabetical rank, and it is blocked twice over. The names are
problem-given data with nowhere to live in this repo, the blocker that also skips
[problem 13](13-large-sum.md) and its neighbours
([kantord/toylang#129](https://github.com/kantord/toylang/issues/129)). Independently of the
data, there is the rank: toylang has no `Vec` sort
([kantord/toylang#86](https://github.com/kantord/toylang/issues/86)),
and every shape a hand-written one takes recurses once per element merged or inserted --
linear in the list's length, not its depth. Five thousand-odd frames is five times the ceiling
that already gates [problem 17](17-number-letter-counts.md) and
[problem 19](19-counting-sundays.md) at one thousand. See the
[spoiler warning](00-spoiler-warning.md).
