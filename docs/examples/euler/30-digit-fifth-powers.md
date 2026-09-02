# Numbers that equal their digits raised to the fifth

Solves [Project Euler 30](https://projecteuler.net/problem=30). See the
[spoiler warning](00-spoiler-warning.md).

The search has a hard ceiling: a six-digit number can be at most `6 * 9^5 = 354294`, itself
six digits, while seven digits can reach only `7 * 9^5 = 413343` -- fewer than the smallest
seven-digit number, `1000000`. Nothing past 354294 can ever equal its own digit-fifth-power
sum, so that is the whole search range. `digit_power_sum` peels digits off from the low end by
repeated `% 10` and `/ 10`, at most six deep for a number this size; the final total sum is
`sum_vec`'s same halving recursion as [problem 21](21-amicable-numbers.md)'s, over however
many numbers pass the filter (six, for fifth powers).

```toylang
fn fifth(d: Int) -> Int = d * d * d * d * d

fn digit_power_sum(n: Int) -> Int =
    n | . == 0 -> 0 or fifth(. % 10) + digit_power_sum(. / 10)

fn is_digit_power_sum(n: Int) -> Bool = digit_power_sum(n) == n

fn sum_vec(p: {v: Vec<Int>, lo: Int, hi: Int}) -> Int =
    p
        | .hi - .lo == 1 -> .v[.lo]! or
              sum_vec({v: .v, lo: .lo, hi: (.lo + .hi) / 2}) + sum_vec({v: .v, lo: (.lo + .hi) / 2, hi: .hi})

fn total(v: Vec<Int>) -> Int =
    v | length(.) == 0 -> 0 or sum_vec({v: v, lo: 0, hi: length(v)})

total(collect(range(354295)) | select(. >= 2) | select(is_digit_power_sum(.)))
```

```output
443839
```

jq is the outlier again, the same as [problem 7](07-10001st-prime.md): scanning 354295
candidates takes it about thirty seconds against a fraction of a second everywhere else. Still
slow, not a wall -- every backend agrees on the answer.
