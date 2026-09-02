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
    p | .n % .d != 0 -> 0 or .d * .d == .n -> .d or .d + .n / .d

fn sigma(p: {n: Int, d: Int}) -> Int =
    p
        | .d * .d > .n -> 0 or
              divisor_contribution({n: .n, d: .d}) + sigma({n: .n, d: .d + 1})

fn proper_divisor_sum(n: Int) -> Int = sigma({n: n, d: 1}) - n

fn is_amicable(n: Int) -> Bool =
    proper_divisor_sum(n) != n and
        proper_divisor_sum(proper_divisor_sum(n)) == n

fn amicable_value(n: Int) -> Int = n | is_amicable(.) -> . or 0

fn sum_range(p: {lo: Int, hi: Int}) -> Int =
    p
        | .hi - .lo == 1 -> amicable_value(.lo) or
              sum_range({lo: .lo, hi: (.lo + .hi) / 2}) + sum_range({lo: (.lo + .hi) / 2, hi: .hi})

sum_range({lo: 1, hi: 10000})
```

```output
31626
```
