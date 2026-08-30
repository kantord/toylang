# The longest Collatz chain under a million (skipped)

Skipped -- but the reason changed. Chain terms overrun 32 bits (432 starting values under a
million pass through a term wider than `Int`, and simulating the old wraparound arithmetic
even changes the winner), and that was the original blocker
([kantord/toylang#38](https://github.com/kantord/toylang/issues/38)). With
[Int64](../../reference/types/int64.md) the terms fit: each chain walked by a tail
recursion over `Int64`, the million starting values compared by a halving recursion,
produces the true winner -- 837799, chain length 525 -- on the compiled backends in a few
seconds, verified while landing
[kantord/toylang#83](https://github.com/kantord/toylang/issues/83).

What stops the page now is cost, not correctness. The million chains are roughly 130 million
recursive steps -- there is no mutation, so nothing memoizes shared chain tails -- and every
docs fragment runs on all seven backends on every `just test`: fine where there is a
compiler, minutes on the interpreted backends. The verified program and the measurements
live with problem 10's in
[kantord/toylang#90](https://github.com/kantord/toylang/issues/90). See the
[spoiler warning](00-spoiler-warning.md).
