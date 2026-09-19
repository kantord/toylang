# Project Euler

**Spoiler warning.** Every page under here is a solution to a numbered
[Project Euler](https://projecteuler.net/) problem, or a note on why there is not one. No
problem statement is reproduced -- each page links to the original and holds only our code --
but the code itself is the spoiler. This section exists for the language, not for teaching:
each solution is a real program the [docs harness](../../reference/syntax/programs.md) runs on
all seven backends, so what you read here is proof of what toylang can express, not a
walkthrough. It stays out of the tutorial and the guides on purpose.

Discussing solutions to the first hundred problems is within Project Euler's own community
norm; this stream stops there.

## Problems 1-10

Solved, one page each: [multiples of 3 and 5](01-multiples-of-3-and-5.md),
[even Fibonacci terms](02-even-fibonacci-sum.md),
[largest prime factor](03-largest-prime-factor.md),
[largest palindrome product](04-largest-palindrome-product.md),
[smallest multiple](05-smallest-multiple.md),
[sum square difference](06-sum-square-difference.md), [the ten-thousand-first prime](07-10001st-prime.md),
[the best window of digits](08-largest-product-in-a-series.md), and
[special Pythagorean triplet](09-special-pythagorean-triplet.md). Problem 3 was unblocked by
[Int64](../../reference/types/int64.md)
([kantord/toylang#83](https://github.com/kantord/toylang/issues/83)), which closed the
too-wide-for-`Int` half of
[kantord/toylang#38](https://github.com/kantord/toylang/issues/38).

[Adding up the primes below a bound](10-summation-of-primes.md) is solved too, as a `slow`
fragment. Two million trial divisions cost the interpreted backends half a minute to minutes,
so [the docs harness](../../reference/syntax/programs.md) type-checks and emits the fragment on
every backend on every `just test` and only executes it under `just slow-test`
([kantord/toylang#90](https://github.com/kantord/toylang/issues/90) asked for that tier;
[kantord/toylang#135](https://github.com/kantord/toylang/issues/135) built it).

## Problems 11-20

Continuing the stream ([kantord/toylang#67](https://github.com/kantord/toylang/issues/67)):
eight solved, two skipped. Three of the solved pages (11, 13, 18) read a blob of problem-given
data that can never live in this repo
([kantord/toylang#39](https://github.com/kantord/toylang/issues/39)); each checks a small
synthetic input in its fragment instead and points at `tests/euler_real_data.rs`, where
`just euler-data DIR` runs the same programs against a contributor's own copies of the real
data and fails loudly on a wrong answer
([kantord/toylang#129](https://github.com/kantord/toylang/issues/129)). An earlier protocol had
these pages read the data from a gitignored fixture and print the commonly published answer
meanwhile, the harness skipping the fragment for everyone who lacked the file -- four pages
asserting a result nobody here had run
([kantord/toylang#69](https://github.com/kantord/toylang/issues/69)). That is undone.

Solved: [the first triangular number with over 500 divisors](12-highly-divisible-triangular-number.md),
[four in a row, multiplied](11-largest-product-in-a-grid.md),
[summing a hundred large numbers](13-large-sum.md),
[the longest Collatz chain under a million](14-longest-collatz-sequence.md),
[counting lattice paths across a grid](15-lattice-paths.md),
[counting letters in one to a thousand](17-number-letter-counts.md),
[the richest way down a triangle](18-maximum-path-sum-i.md), and
[Sundays on the first of the month, 1901-2000](19-counting-sundays.md). Problem 14 is a `slow`
fragment like problem 10: the million chains price the interpreted backends out of the
every-fragment suite, so `just test` compiles the fragment on every backend and `just
slow-test` runs it
([kantord/toylang#90](https://github.com/kantord/toylang/issues/90)). Problem 15 waited on
Int64 too: its answer, about 1.4e11, is two orders of magnitude past `Int`. Problems 8, 13 and
18 are confirmed against the real, official data; problem 11's real-sized run tripped the
Python backend's default 1000-frame recursion limit, which
[kantord/toylang#132](https://github.com/kantord/toylang/issues/132) raised to 100000 in
`emit_py.rs`, and no confirmation has been recorded since.

Still skipped: [**Problem 16**](16-power-digit-sum.md) and
[**Problem 20**](20-factorial-digit-sum.md) need arbitrary-precision arithmetic -- `2^1000`
and `100!` are hundreds of digits, past any fixed width -- which
[kantord/toylang#112](https://github.com/kantord/toylang/issues/112) tracks and deliberately
leaves unscheduled.

## Problems 21-30

Continuing the stream ([kantord/toylang#92](https://github.com/kantord/toylang/issues/92)):
six solved, four skipped, none of them for width.

Solved: [the pairs that sum to each other](21-amicable-numbers.md),
[the millionth lexicographic permutation of 0123456789](24-lexicographic-permutations.md),
[the unit fraction with the longest repeating cycle](26-reciprocal-cycles.md),
[summing the corners of a number spiral](28-number-spiral-diagonals.md),
[how many powers are truly distinct](29-distinct-powers.md) -- without big integers, by
rewriting each power as a `(root, exponent)` pair instead of computing it -- and
[numbers that equal their digits raised to the fifth](30-digit-fifth-powers.md).

Still skipped:

- [**Problem 22**](22-names-scores.md): its names are problem-given data with nowhere to live
  here, though the opt-in protocol of
  [kantord/toylang#129](https://github.com/kantord/toylang/issues/129) could carry them as it
  does problems 13 and 18, and the `Vec` sort it also once waited on exists
  ([kantord/toylang#86](https://github.com/kantord/toylang/issues/86)). Nobody has written
  the page.
- [**Problem 23**](23-non-abundant-sums.md) and [**Problem 27**](27-quadratic-primes.md) solve
  correctly on every backend but search too widely for the every-fragment suite;
  [kantord/toylang#93](https://github.com/kantord/toylang/issues/93) records the timings, and a
  maintainer ruling there parks them for the slow-fragment tier.
- [**Problem 25**](25-1000-digit-fibonacci-number.md) needs arbitrary-precision integers
  ([kantord/toylang#112](https://github.com/kantord/toylang/issues/112)), the one problem 29's
  trick could not also dodge.
