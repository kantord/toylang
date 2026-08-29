# Counting lattice paths across a grid (skipped)

Skipped. [Project Euler 15](https://projecteuler.net/problem=15) asks for the number of
right-and-down paths across a 20x20 grid, which is `C(40,20)` = 137,846,528,820 -- past
`Int`'s roughly 2.1e9 ceiling by two orders of magnitude, and no smaller intermediate along
the way stays under it either, since it is the largest entry the count ever reaches. See
[kantord/toylang#38](https://github.com/kantord/toylang/issues/38) and the
[spoiler warning](00-spoiler-warning.md).
