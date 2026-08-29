# Difference between the sum of squares and the square of the sum

Solves [Project Euler 6](https://projecteuler.net/problem=6). See the
[spoiler warning](00-spoiler-warning.md).

Both sums are closed forms (triangular numbers, square pyramidal numbers), so nothing here
iterates at all.

```toylang
fn tri(n: Int) -> Int = n * (n + 1) / 2

fn sumsq(n: Int) -> Int = n * (n + 1) * (2 * n + 1) / 6

fn square(x: Int) -> Int = x * x

square(tri(100)) - sumsq(100)
```

```output
25164150
```
