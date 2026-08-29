# Project Euler

**Spoiler warning.** Every page under here is a solution to a numbered
[Project Euler](https://projecteuler.net/) problem. No problem statement is reproduced --
each page links to the original and holds only our code -- but the code itself is the
spoiler. This section exists for the language, not for teaching: each solution is a real
program the [docs harness](../../reference/syntax/programs.md) runs on all seven backends,
so what you read here is proof of what toylang can express, not a walkthrough. It stays out
of the tutorial and the guides on purpose.

Discussing solutions to the first hundred problems is within Project Euler's own community
norm; this stream stops there.

## Problems 1-10

Solved, one page each: [multiples of 3 and 5](01-multiples-of-3-and-5.md),
[even Fibonacci terms](02-even-fibonacci-sum.md),
[largest palindrome product](04-largest-palindrome-product.md),
[smallest multiple](05-smallest-multiple.md),
[sum square difference](06-sum-square-difference.md), [the ten-thousand-first prime](07-10001st-prime.md),
and [special Pythagorean triplet](09-special-pythagorean-triplet.md).

Three are skipped, each for a real reason found while trying to express it rather than
avoided in advance:

- [**Problem 3**](03-largest-prime-factor.md), [**Problem 8**](08-largest-product-in-a-series.md),
  and [**Problem 10**](10-summation-of-primes.md) need an integer wider than toylang's 32-bit
  `Int` carries -- to state the input, to hold the correct sum, or (problem 8) because the
  largest product of 13 digits is `9^13`, past `Int` regardless of how the input arrives.
  Filed as [kantord/toylang#38](https://github.com/kantord/toylang/issues/38).

Problem 8's other open question, whether a problem-given data blob is fair to embed at all, is
settled: see problems 11-20 below.

## Problems 11-20

Continuing the stream ([kantord/toylang#67](https://github.com/kantord/toylang/issues/67)):
six solved, four skipped. [kantord/toylang#39](https://github.com/kantord/toylang/issues/39)
settled the data question that had blocked three of them (and, in hindsight, problem 8 from
batch 1): a problem-given blob arrives on stdin from a gitignored fixture, never committed to
the repo, and the docs harness runs the fragment only when a contributor has supplied their
own copy -- skipping it, not failing, otherwise. What remains skipped in this batch all needs
more than 32 bits instead.

Solved: [the first triangular number with over 500 divisors](12-highly-divisible-triangular-number.md),
[counting letters in one to a thousand](17-number-letter-counts.md),
[Sundays on the first of the month, 1901-2000](19-counting-sundays.md),
[largest product in a grid](11-largest-product-in-a-grid.md),
[summing a hundred large numbers](13-large-sum.md), and
[the richest way down a triangle](18-maximum-path-sum-i.md) -- the last three gated on the
fixture protocol above, per [kantord/toylang#69](https://github.com/kantord/toylang/issues/69).

Skipped for [kantord/toylang#38](https://github.com/kantord/toylang/issues/38) (needs more
than 32 bits): [**Problem 14**](14-longest-collatz-sequence.md),
[**Problem 15**](15-lattice-paths.md), [**Problem 16**](16-power-digit-sum.md), and
[**Problem 20**](20-factorial-digit-sum.md).
