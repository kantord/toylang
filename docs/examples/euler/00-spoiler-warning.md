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

One is skipped, and not for anything the language lacks:

- [**Problem 10**](10-summation-of-primes.md) now produces the right answer on all seven
  backends, but two million trial divisions cost the interpreted backends minutes, and every
  fragment here runs on every `just test`. Whether the docs harness grows a slow tier is
  [kantord/toylang#90](https://github.com/kantord/toylang/issues/90).

## Problems 11-20

Continuing the stream ([kantord/toylang#67](https://github.com/kantord/toylang/issues/67)):
six solved, four skipped. Three of the solved pages (11, 13, 18) read a blob of problem-given
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
[counting letters in one to a thousand](17-number-letter-counts.md),
[the richest way down a triangle](18-maximum-path-sum-i.md), and
[Sundays on the first of the month, 1901-2000](19-counting-sundays.md). Problems 8, 13 and 18
are confirmed against the real, official data; problem 11 is not yet, because at the real 20x20
scale its linear maximum scan blows past the Python backend's 1000-frame recursion limit
([kantord/toylang#132](https://github.com/kantord/toylang/issues/132)), a gap in `emit_py.rs`,
not in the language.

Still skipped:

- [**Problem 14**](14-longest-collatz-sequence.md): the chain terms fit
  [Int64](../../reference/types/int64.md) now and the true winner comes out on the compiled
  backends, but roughly 130 million recursive steps price the interpreted backends out of
  the every-fragment suite -- the same slow-tier question as problem 10,
  [kantord/toylang#90](https://github.com/kantord/toylang/issues/90).
- [**Problem 15**](15-lattice-paths.md): its answer (about 1.4e11) fits Int64, so the width
  blocker recorded under [kantord/toylang#38](https://github.com/kantord/toylang/issues/38)
  no longer applies; nobody has written the page since.
- [**Problem 16**](16-power-digit-sum.md) and
  [**Problem 20**](20-factorial-digit-sum.md) need arbitrary-precision arithmetic -- `2^1000`
  and `100!` are hundreds of digits, past any fixed width -- which is the half of
  [kantord/toylang#38](https://github.com/kantord/toylang/issues/38) that stays open.
