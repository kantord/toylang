# Digit sum of a huge power

Solves [Project Euler 16](https://projecteuler.net/problem=16). See the
[spoiler warning](00-spoiler-warning.md).

2^1000 is 302 digits long, past `Int64` by 283 of them, but the problem never asks for the
number, only for its digits, and those fit in a `Vec<Int>` the way [problem 13](13-large-sum.md)
keeps its sum. `scale` multiplies such a vector by a small `k` one column at a time from the
least significant end (the digits are stored little-endian so a new leading digit is an append,
not a prepend), and `push_carry` spills whatever carry is left over into as many further digits
as it needs. Doubling is the case `k == 2`: a column total is at most `2 * 9 + 1`, so nothing
here approaches `Int`'s ceiling even though the number as a whole is nowhere near it.
`power_of_two` doubles a thousand times, each doubling a self-tail-call, and `sum` reads the
answer straight off the digits.

What arbitrary-precision integers would still add is the number itself: this page can sum the
digits of 2^1000 but not print 2^1000, or compare it, or divide it by anything. That is what
[kantord/toylang#112](https://github.com/kantord/toylang/issues/112) tracks, and it is not what
this problem needs.

```toylang
fn push_carry(
  { carry, acc }: { carry: Int, acc: Vec<Int> }
) -> Vec<Int> =
  carry == 0
  | . -> acc or
    push_carry { carry: carry / 10, acc: acc + [carry % 10] }

fn scale(
  { digits, k, i, carry, acc }: {
    digits: Vec<Int>,
    k: Int,
    i: Int,
    carry: Int,
    acc: Vec<Int>
  }
) -> Vec<Int> =
  let total = digits[i]! * k + carry
  i == length digits - 1
  | . -> push_carry { carry: total / 10, acc: acc + [total % 10] } or
    scale {
      digits: digits,
      k: k,
      i: i + 1,
      carry: total / 10,
      acc: acc + [total % 10]
    }

fn power_of_two(
  { digits, n }: { digits: Vec<Int>, n: Int }
) -> Vec<Int> =
  n == 0
  | . -> digits or
    power_of_two {
      digits:
        scale { digits: digits, k: 2, i: 0, carry: 0, acc: [] },
      n: n - 1
    }

sum(power_of_two { digits: [1], n: 1000 })
```

```output
1366
```
