# Non-abundant sums (skipped)

Skipped. [Project Euler 23](https://projecteuler.net/problem=23) asks for every number under
28123 that is not the sum of two abundant numbers, and the search for whether `k` is such a
sum is where this stops: with no sorted `Vec` and no set membership faster than a linear scan
([kantord/toylang#86](https://github.com/kantord/toylang/issues/86)), checking one `k` means
scanning the abundant-number list up to `k/2`. It is not a wall -- a reduced run (bound 2000,
732 non-abundant numbers) is instant and the full bound (28123, 1456 non-abundant numbers)
comes back correct too -- but the full search takes 42 seconds on Go alone, the fastest of
the seven backends. See [kantord/toylang#93](https://github.com/kantord/toylang/issues/93)
and the [spoiler warning](00-spoiler-warning.md).
