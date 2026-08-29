# The longest Collatz chain under a million (skipped)

Skipped. [Project Euler 14](https://projecteuler.net/problem=14) allows chain terms to run
past the starting bound, and they do: 432 of the starting values under a million pass through
a term wider than `Int`'s 32-bit ceiling somewhere in their chain (the eventual winner's own
chain peaks at 2,974,984,576). This is not just a misreported peak: simulating the same
32-bit wraparound `Int` arithmetic uses gives a different winner (626331, chain length 489)
than the true answer (837799, chain length 525), so a wrapped solution would compile,
run, agree across backends, and print a wrong number with nothing to flag it. See
[kantord/toylang#38](https://github.com/kantord/toylang/issues/38) and the
[spoiler warning](00-spoiler-warning.md).
