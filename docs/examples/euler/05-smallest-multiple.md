# Smallest number divisible by 1 through 20

Solves [Project Euler 5](https://projecteuler.net/problem=5). See the
[spoiler warning](00-spoiler-warning.md).

`gcd` is the textbook Euclidean recursion; `lcm` and `lcm_upto` build on it, folding the
range 1 to 20 from the top down.

```toylang
fn gcd({ a, b }: { a: Int, b: Int }) -> Int =
  b == 0 | . -> a or gcd { a: b, b: a % b }

fn lcm({ a, b }: { a: Int, b: Int }) -> Int =
  a / gcd { a: a, b: b } * b

fn lcm_upto({ n, limit }: { n: Int, limit: Int }) -> Int =
  n > limit
  | . -> 1 or lcm { a: lcm_upto { n: n + 1, limit: limit }, b: n }

lcm_upto { n: 1, limit: 20 }
```

```output
232792560
```
