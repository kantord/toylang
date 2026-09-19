# Summing a hundred large numbers

Solves [Project Euler 13](https://projecteuler.net/problem=13). See the
[spoiler warning](00-spoiler-warning.md).

The hundred 50-digit numbers are problem-given data
([kantord/toylang#39](https://github.com/kantord/toylang/issues/39)); the fragment below sums a
synthetic set of three ten-digit numbers, and the real-sized check lives in
`tests/euler_real_data.rs`, opt-in via `just euler-data DIR`.

Neither the input nor the sum fits [`Int`](../../reference/types/int.md), which is 32 bits, so
`add_digits` adds one column of digits at a time from the right, the way it is done on paper.
`column_total` uses the `sum` builtin directly over each row's digit at that column; every
column total -- at most a hundred nines plus a small carry -- stays far inside `Int` even
though the sum as a whole does not, and only the leading ten digits the problem asks for are
kept, a `Vec<Int>` of digits rather than a number nothing here could hold.
[Problem 24](24-lexicographic-permutations.md) reaches for the same digits-in-a-`Vec`
representation.

The example's three numbers, two of them all nines, ripple a carry all the way up, so the sum is
20000000000 and the answer is its first ten digits.

```toylang
fn column_total(
  { nums, k, carry }: { nums: Vec<Vec<Int>>, k: Int, carry: Int }
) -> Int =
  sum(nums | map(.[k]!)) + carry


fn emit_carry(
  { carry, acc }: { carry: Int, acc: Vec<Int> }
) -> Vec<Int> =
  carry == 0
  | . -> acc or
    emit_carry { carry: carry / 10, acc: [carry % 10] + acc }


fn add_digits(
  { nums, k, carry, acc }: {
    nums: Vec<Vec<Int>>,
    k: Int,
    carry: Int,
    acc: Vec<Int>
  }
) -> Vec<Int> =
  let total = column_total { nums: nums, k: k, carry: carry }

  k == 0
  | . -> emit_carry { carry: total / 10, acc: [total % 10] + acc } or
    add_digits {
      nums: nums,
      k: k - 1,
      carry: total / 10,
      acc: [total % 10] + acc
    }


fn leading_digits(nums: Vec<Vec<Int>>) -> Vec<Int> =
  add_digits {
    nums: nums,
    k: length(nums[0]!) - 1,
    carry: 0,
    acc: []
  }[0:10]


leading_digits(parse stdin)
```

```input
[[9,9,9,9,9,9,9,9,9,9],[9,9,9,9,9,9,9,9,9,9],[0,0,0,0,0,0,0,0,0,2]]
```

```output
[2,0,0,0,0,0,0,0,0,0]
```
