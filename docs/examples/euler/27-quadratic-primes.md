# The quadratic that mints the longest prime run (skipped)

Skipped. [Project Euler 27](https://projecteuler.net/problem=27) asks which coefficients
`a, b` (with `|a| < 1000`, `|b| <= 1000`) make `n*n + a*n + b` prime for the longest run of
consecutive `n` starting at zero, and the correct product (`-59231`, from `a=-61, b=971`) does
come back on every backend. What rules it out is cost, not correctness: checking every
coefficient pair against every prime `b` up to 1000 is 335,832 cells, each running its own
consecutive-prime count, and on jq that took 39 seconds even after removing an accidental
double search. See [kantord/toylang#93](https://github.com/kantord/toylang/issues/93) and the
[spoiler warning](00-spoiler-warning.md).
