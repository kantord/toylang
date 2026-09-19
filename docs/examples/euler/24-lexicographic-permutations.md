# The millionth lexicographic permutation of 0123456789

Solves [Project Euler 24](https://projecteuler.net/problem=24). See the
[spoiler warning](00-spoiler-warning.md).

No search: the factorial number system picks each digit directly. With `k` digits still
unplaced, the next `(k-1)!` permutations share the first remaining digit, so dividing the
target index by `(k-1)!` gives that digit's position `i` in what's left, and the remainder
carries into the next digit; the slices `remaining[:i] + remaining[i + 1:]` are the list with
that digit dropped. Ten digits keep the recursion to depth ten. Joined, the digits of the
millionth permutation (index 999999, since the first is index zero) make 2783915460, past
`Int` though inside [Int64](../../reference/types/int64.md), so `join_digits` folds them into
an `Int64` accumulator, `acc * 10 + i64(d)`, and the answer prints as the number it is rather
than as the `Vec<Int>` of digits [problem 13](13-large-sum.md) has to settle for.

```toylang
fn factorial(n: Int) -> Int = n | . <= 1 -> 1 or . * factorial(. - 1)


fn nth_perm(
  { remaining, idx }: { remaining: Vec<Int>, idx: Int }
) -> Vec<Int> =
  let block = factorial(length(remaining) - 1)
  let i = idx / block

  remaining
  | length(remaining) == 0 -> [] or
    [remaining[i]!] +
      nth_perm {
        remaining: remaining[:i] + remaining[i + 1:],
        idx: idx % block
      }


fn join_digits(
  { digits, acc }: { digits: Vec<Int>, acc: Int64 }
) -> Int64 =
  length(digits) == 0
  | . -> acc or
    join_digits {
      digits: tail(digits)!,
      acc: acc * 10 + i64(digits[0]!)
    }


fn as_number(digits: Vec<Int>) -> Int64 =
  join_digits { digits: digits, acc: 0 }


as_number(
  nth_perm { remaining: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], idx: 999999 }
)
```

```output
2783915460
```
