# The richest way down a triangle (skipped)

Skipped. The 15-row triangle is problem-given data
([kantord/toylang#39](https://github.com/kantord/toylang/issues/39)), so the best total has never
been computed here. [Problem 8](08-largest-product-in-a-series.md) has the account of how these
pages came to publish one anyway; the program is in
[kantord/toylang#129](https://github.com/kantord/toylang/issues/129).

The approach is worth a line because the language had nothing to do with the choice: folding from
the bottom up, replacing each row with that entry plus the larger of the two below it, until row 0
holds one number. Branching top-down instead would revisit the same cell many times over. See the
[spoiler warning](00-spoiler-warning.md).
