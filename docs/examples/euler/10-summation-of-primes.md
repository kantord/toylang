# Adding up the primes below a bound (skipped)

Skipped -- but the reason changed. [Project Euler 10](https://projecteuler.net/problem=10)
asks for a sum on the order of 1.4e11, past `Int`'s roughly 2.1e9 ceiling, and that was the
original blocker ([kantord/toylang#38](https://github.com/kantord/toylang/issues/38)). With
[Int64](../../reference/types/int64.md) the width problem is gone: trial division over
`range(2000000)` in [problem 7](07-10001st-prime.md)'s shape, with the selected primes summed
by a halving recursion into an `Int64`, produces the correct answer on all seven backends --
verified while landing [kantord/toylang#83](https://github.com/kantord/toylang/issues/83).

What stops the page now is cost, not correctness. Every fragment in these docs runs on all
seven backends on every `just test`, and two million trial divisions are cheap only where
there is a compiler: about a second on Go and V8, half a minute on CPython, a minute on Lua,
and longer still on jq, against a whole test suite that otherwise finishes in about ninety
seconds. Whether the docs harness should grow a tier for slow fragments -- or this page
should simply pay the price -- is
[kantord/toylang#90](https://github.com/kantord/toylang/issues/90)'s question, and the
verified program lives there. See the [spoiler warning](00-spoiler-warning.md).
