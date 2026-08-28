# Multiples of 3 and 5 below 1000

Solves [Project Euler 1](https://projecteuler.net/problem=1). See the
[spoiler warning](00-spoiler-warning.md).

No fold or loop needed: the sum of the multiples of `k` below `limit` is a triangular number,
scaled, and inclusion-exclusion handles the double-counted multiples of 15.

```toylang
fn triangle(m: Int) -> Int = m * (m + 1) / 2

fn sum_of_multiples(p: {k: Int, limit: Int}) -> Int =
    triangle((p.limit - 1) / p.k) * p.k

sum_of_multiples({k: 3, limit: 1000}) + sum_of_multiples({k: 5, limit: 1000}) -
    sum_of_multiples({k: 15, limit: 1000})
```

```output
233168
```
