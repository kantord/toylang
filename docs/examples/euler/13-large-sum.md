# Summing a hundred large numbers (skipped)

Skipped. The hundred 50-digit numbers are problem-given data
([kantord/toylang#39](https://github.com/kantord/toylang/issues/39)), so the leading ten digits of
their sum have never been computed here. [Problem 8](08-largest-product-in-a-series.md) has the
account of how these pages came to publish an answer anyway; the program is in
[kantord/toylang#129](https://github.com/kantord/toylang/issues/129).

One thing from it outlives the page. Neither the input nor the sum fits `Int`, which is 32 bits
([kantord/toylang#38](https://github.com/kantord/toylang/issues/38)), and the way around that was
not a wider type but adding one column of digits at a time from the right, the way it is done on
paper. Every column total -- at most a hundred nines plus a small carry -- stays far inside `Int`
even though the sum as a whole does not, and only the leading ten digits the problem asks for are
kept, a `Vec<Int>` of digits rather than a number nothing here could hold.
[Problem 24](24-lexicographic-permutations.md) reaches for the same digits-in-a-`Vec`
representation. See the [spoiler warning](00-spoiler-warning.md).
