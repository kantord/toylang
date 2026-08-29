# Smallest number divisible by 1 through 20

Solves [Project Euler 5](https://projecteuler.net/problem=5). See the
[spoiler warning](00-spoiler-warning.md).

`gcd` is the textbook Euclidean recursion; `lcm` and `lcm_upto` build on it, folding the
range 1 to 20 from the top down.

```toylang
fn gcd(p: {a: Int, b: Int}) -> Int =
    p.a if p.b == 0 else gcd({a: p.b, b: p.a % p.b})

fn lcm(p: {a: Int, b: Int}) -> Int = p.a / gcd({a: p.a, b: p.b}) * p.b

fn lcm_upto(p: {n: Int, limit: Int}) -> Int =
    1 if p.n > p.limit else
        lcm({a: lcm_upto({n: p.n + 1, limit: p.limit}), b: p.n})

lcm_upto({n: 1, limit: 20})
```

```output
232792560
```
