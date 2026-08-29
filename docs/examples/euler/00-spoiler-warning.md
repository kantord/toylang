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

- [**Problem 3**](03-largest-prime-factor.md) and [**Problem 10**](10-summation-of-primes.md)
  need an integer wider than toylang's 32-bit `Int` carries -- one to state the input, the
  other to hold the correct sum. Filed as
  [kantord/toylang#38](https://github.com/kantord/toylang/issues/38).
- [**Problem 8**](08-largest-product-in-a-series.md) hands the solver a 1000-digit number
  that has no source but Project Euler itself; whether a data blob like that is fair to embed
  under the no-Euler-text rule is an open call. Filed as
  [kantord/toylang#39](https://github.com/kantord/toylang/issues/39).

## Problems 11-20

Continuing the stream ([kantord/toylang#67](https://github.com/kantord/toylang/issues/67)):
three solved, seven skipped. The skips lean almost entirely on the same two open gates batch
1 found, rather than new ones -- this stretch of the problem set asks for wide integers and
verbatim inputs more often than it asks for anything toylang cannot otherwise express.

Solved: [the first triangular number with over 500 divisors](12-highly-divisible-triangular-number.md),
[counting letters in one to a thousand](17-number-letter-counts.md), and
[Sundays on the first of the month, 1901-2000](19-counting-sundays.md).

Skipped for [kantord/toylang#38](https://github.com/kantord/toylang/issues/38) (needs more
than 32 bits): [**Problem 14**](14-longest-collatz-sequence.md),
[**Problem 15**](15-lattice-paths.md), [**Problem 16**](16-power-digit-sum.md), and
[**Problem 20**](20-factorial-digit-sum.md).

Skipped for [kantord/toylang#39](https://github.com/kantord/toylang/issues/39) (verbatim
problem-specific data with no other source): [**Problem 11**](11-largest-product-in-a-grid.md),
[**Problem 13**](13-large-sum.md), and [**Problem 18**](18-maximum-path-sum-i.md).
