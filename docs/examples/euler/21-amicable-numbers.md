# The pairs that sum to each other

Solves [Project Euler 21](https://projecteuler.net/problem=21). See the
[spoiler warning](00-spoiler-warning.md).

`sigma` sums a number's divisors (itself included) by pairing `d` with `n/d` up to `sqrt(n)`,
the same trick that keeps [problem 12](12-highly-divisible-triangular-number.md)'s divisor
count cheap; subtracting `n` turns it into the sum of *proper* divisors the problem asks for.
Two numbers are amicable when each is the other's proper-divisor sum and neither is itself
(ruling out perfect numbers, which are their own answer). `sum_range` halves its way down to
single numbers the same way [problem 90](https://github.com/kantord/toylang/issues/90)'s
sample programs do, keeping recursion depth at `log2(10000)` rather than one frame per number.

```toylang
fn divisor_contribution(p: {n: Int, d: Int}) -> Int =
    0 if p.n % p.d != 0 else p.d if p.d * p.d == p.n else p.d + p.n / p.d

fn sigma(p: {n: Int, d: Int}) -> Int =
    0 if p.d * p.d > p.n else
        divisor_contribution({n: p.n, d: p.d}) + sigma({n: p.n, d: p.d + 1})

fn proper_divisor_sum(n: Int) -> Int = sigma({n: n, d: 1}) - n

fn is_amicable(n: Int) -> Bool =
    proper_divisor_sum(proper_divisor_sum(n)) == n if proper_divisor_sum(n) != n else
        1 == 0

fn amicable_value(n: Int) -> Int = n if is_amicable(n) else 0

fn sum_range(p: {lo: Int, hi: Int}) -> Int =
    amicable_value(p.lo) if p.hi - p.lo == 1 else
        sum_range({lo: p.lo, hi: (p.lo + p.hi) / 2}) +
            sum_range({lo: (p.lo + p.hi) / 2, hi: p.hi})

sum_range({lo: 1, hi: 10000})
```

```output
31626
```
