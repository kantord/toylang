# The first triangular number with over 500 divisors

Solves [Project Euler 12](https://projecteuler.net/problem=12). See the
[spoiler warning](00-spoiler-warning.md).

Counting a triangular number's own divisors by trial division would mean factoring numbers up
around 76 million; instead this factors the two numbers that build it. `n` and `n+1` share no
factor, and `n*(n+1)/2` is one of them times the other's half, so its divisor count is the
product of `count_divisors(n/2)` and `count_divisors(n+1)` (or the odd-`n` mirror) -- trial
division stays under a few hundred either way. `range` with a bound known to hold the answer
takes the place of open-ended search, the same shape as [the ten-thousand-first
prime](07-10001st-prime.md).

```toylang
fn cd_loop(p: {m: Int, d: Int, count: Int}) -> Int =
    p
        | .d * .d > .m -> p.count or
              cd_loop({m: p.m, d: p.d + 1, count: p.count + (p | .m % .d == 0 -> 2 or 0) - (p | .d * .d == .m -> 1 or 0)})

fn count_divisors(m: Int) -> Int = cd_loop({m: m, d: 1, count: 0})

fn triangle_divisors(n: Int) -> Int =
    n
        | . % 2 == 0 -> count_divisors(n / 2) * count_divisors(n + 1) or
              count_divisors(n) * count_divisors((n + 1) / 2)

fn triangle(n: Int) -> Int = n * (n + 1) / 2

triangle(
    (collect(range(12376)) | select(. >= 1) | select(triangle_divisors(.) > 500))[0]!
)
```

```output
76576500
```

Trial division inside `select` is still the expensive part: jq takes on the order of fifteen
seconds against a fraction of a second everywhere else, the same outlier it was for problem 7.
