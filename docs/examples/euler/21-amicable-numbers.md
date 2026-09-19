# The pairs that sum to each other

Solves [Project Euler 21](https://projecteuler.net/problem=21). See the
[spoiler warning](00-spoiler-warning.md).

`sigma` sums a number's divisors (itself included) by pairing `d` with `n/d` up to `sqrt(n)`,
the same trick that keeps [problem 12](12-highly-divisible-triangular-number.md)'s divisor
count cheap; subtracting `n` turns it into the sum of *proper* divisors the problem asks for.
Two numbers are amicable when each is the other's proper-divisor sum and neither is itself
(ruling out perfect numbers, which are their own answer), and the answer is `sum` over the
range filtered by `is_amicable`.

```toylang
fn divisor_contribution({ n, d }: { n: Int, d: Int }) -> Int =
  d | n % d != 0 -> 0 or d * d == n -> d or d + n / d

fn sigma({ n, d }: { n: Int, d: Int }) -> Int =
  d
  | d * d > n -> 0 or
    divisor_contribution({ n: n, d: d }) + sigma({ n: n, d: d + 1 })

fn proper_divisor_sum(n: Int) -> Int = sigma({ n: n, d: 1 }) - n

fn is_amicable(n: Int) -> Bool =
  proper_divisor_sum(n) != n and
    proper_divisor_sum(proper_divisor_sum(n)) == n

sum(collect(range(10000)) | select(is_amicable(.)))
```

```output
31626
```
