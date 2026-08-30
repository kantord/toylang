# Four in a row, multiplied (skipped)

Skipped. The 20x20 grid is problem-given data, so under
[kantord/toylang#39](https://github.com/kantord/toylang/issues/39) it cannot live here, and a
product computed from a grid nobody in this repo has is not a checked answer.
[Problem 8](08-largest-product-in-a-series.md) has the account of how these pages came to publish
one anyway; the program is in
[kantord/toylang#129](https://github.com/kantord/toylang/issues/129).

Nothing about the language blocks it. The four directions are ordinary `range` and `map` work
over a `Vec<Vec<Int>>`, bounded so no step reaches outside the grid, and the largest product,
about 7.1e7, is comfortably inside `Int`. See the [spoiler warning](00-spoiler-warning.md).
