# The best window of digits (skipped)

Skipped. [Project Euler 8](https://projecteuler.net/problem=8) no longer stops here on the
data question: [kantord/toylang#39](https://github.com/kantord/toylang/issues/39) settled that
a problem-given blob like this one arrives on stdin from a gitignored fixture, the protocol
[problem 11](11-largest-product-in-a-grid.md), [problem 13](13-large-sum.md), and
[problem 18](18-maximum-path-sum-i.md) now use. What stops it here is `Int`, which is 32 bits:
the largest product of any 13 digits is `9^13`, 2,541,865,828,329, and nothing close to it fits
in `Int`'s roughly 2.1 billion ceiling.

Problem 13 dodges the same ceiling by summing one column of digits at a time, each column
total small enough for `Int` on its own. A product has the same shape in principle -- multiply
the digit array by one more digit, carrying, the way long multiplication does it by hand -- but
that is a carry-propagating multiply-and-compare routine over arbitrary-width numbers, which is
arbitrary-precision arithmetic as a feature, not a trick specific to this page. Building that to
answer one example is the general work
[kantord/toylang#38](https://github.com/kantord/toylang/issues/38) already owns. Filed there,
and the [spoiler warning](00-spoiler-warning.md).
