# Names scores (skipped)

Skipped. [Project Euler 22](https://projecteuler.net/problem=22) scores a text file of over
five thousand names by alphabetical rank, and the data half is ordinary: `Str` already orders
by codepoint, so the names could arrive as a [runtime fixture](13-large-sum.md) the same way
the 11-20 batch's data problems do. What blocks it is the rank itself, not the reading:
toylang has no `Vec` sort ([kantord/toylang#86](https://github.com/kantord/toylang/issues/86)),
and every shape a hand-written one takes recurses once per element merged or inserted --
linear in the list's length, not its depth. Five thousand-odd frames is five times the ceiling
that already gates [problem 17](17-number-letter-counts.md) and
[problem 19](19-counting-sundays.md) at one thousand. See the
[spoiler warning](00-spoiler-warning.md).
