# Summation of primes (skipped)

Skipped. [Project Euler 10](https://projecteuler.net/problem=10) asks for a sum that overflows
toylang's 32-bit `Int` -- the true answer is on the order of 1.4e11, past `Int`'s roughly
2.1e9 ceiling. `Int` wraps rather than trapping, so a solution would compile, run, and every
backend would agree on the same wrong, wrapped value. See
[kantord/toylang#38](https://github.com/kantord/toylang/issues/38) and the
[spoiler warning](00-spoiler-warning.md).
