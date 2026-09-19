# What two abundant numbers cannot reach (skipped)

Skipped. [Project Euler 23](https://projecteuler.net/problem=23) asks for every number under
28123 that is not the sum of two abundant numbers, and the search for whether `k` is such a
sum is where this stops: there is no set type, so checking one `k` means walking the
abundant-number list up to `k/2`. It is not a wall -- a reduced run (bound 2000, 732
non-abundant numbers) is instant and the full bound (28123, 1456 non-abundant numbers) comes
back correct too -- but the full search took 42 seconds on Go alone, the fastest of the seven
backends. [kantord/toylang#93](https://github.com/kantord/toylang/issues/93) records the
timings, and a maintainer ruling there parks the page for the slow-fragment tier rather than
the every-fragment suite. See the [spoiler warning](00-spoiler-warning.md).
