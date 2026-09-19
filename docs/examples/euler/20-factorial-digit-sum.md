# Digit sum of a huge factorial

Solves [Project Euler 20](https://projecteuler.net/problem=20). See the
[spoiler warning](00-spoiler-warning.md).

The same digit vector as [problem 16](16-power-digit-sum.md), with the same `scale` and
`push_carry`; only the multiplier changes. `factorial_digits` multiplies the little-endian
digits by every `k` from 2 to 100 in turn, so 100! is built up as 1, 2, 6, 24, ... without ever
being a number. The column totals grow with `k` but stay small: a digit times 100 plus a carry
under 100 is under 1000, far inside `Int`, while the product itself reaches 158 digits.

```toylang
fn push_carry({carry, acc}: {carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    carry == 0
        | . -> acc or push_carry({carry: carry / 10, acc: acc + [carry % 10]})

fn scale({digits, k, i, carry, acc}: {digits: Vec<Int>, k: Int, i: Int, carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    let total = digits[i]! * k + carry
    i == length(digits) - 1
        | . -> push_carry({carry: total / 10, acc: acc + [total % 10]}) or
              scale({digits: digits, k: k, i: i + 1, carry: total / 10, acc: acc + [total % 10]})

fn factorial_digits({digits, k}: {digits: Vec<Int>, k: Int}) -> Vec<Int> =
    k > 100
        | . -> digits or
              factorial_digits({digits: scale({digits: digits, k: k, i: 0, carry: 0, acc: []}), k: k + 1})

sum(factorial_digits({digits: [1], k: 2}))
```

```output
648
```
