# Summing a hundred large numbers

Solves [Project Euler 13](https://projecteuler.net/problem=13). See the
[spoiler warning](00-spoiler-warning.md).

The hundred 50-digit numbers are problem-given data
([kantord/toylang#39](https://github.com/kantord/toylang/issues/39)); the fragment below sums a
synthetic set of three ten-digit numbers, and the real-sized check lives in
`tests/euler_real_data.rs`, opt-in via `just euler-data DIR`.

Neither the input nor the sum fits `Int`, which is 32 bits
([kantord/toylang#38](https://github.com/kantord/toylang/issues/38)), so `add_digits` adds one
column of digits at a time from the right, the way it is done on paper. Every column total -- at
most a hundred nines plus a small carry -- stays far inside `Int` even though the sum as a whole
does not, and only the leading ten digits the problem asks for are kept, a `Vec<Int>` of digits
rather than a number nothing here could hold.
[Problem 24](24-lexicographic-permutations.md) reaches for the same digits-in-a-`Vec`
representation.

The example's three numbers, two of them all nines, ripple a carry all the way up, so the sum is
20000000000 and the answer is its first ten digits.

```toylang
fn empty() -> Vec<Int> = []

fn col_sum(p: {nums: Vec<Vec<Int>>, i: Int, k: Int}) -> Int =
    p
        | .i >= length(.nums) -> 0 or
              p.nums[p.i]![p.k]! + col_sum({nums: p.nums, i: p.i + 1, k: p.k})

fn column_total(p: {nums: Vec<Vec<Int>>, k: Int, carry: Int}) -> Int =
    col_sum({nums: p.nums, i: 0, k: p.k}) + p.carry

fn emit_carry(p: {carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    p
        | .carry == 0 -> p.acc or
              emit_carry({carry: p.carry / 10, acc: [p.carry % 10] + p.acc})

fn add_digits(p: {nums: Vec<Vec<Int>>, k: Int, carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    p
        | .k < 0 -> emit_carry({carry: p.carry, acc: p.acc}) or
              add_digits({nums: p.nums, k: p.k - 1, carry: column_total({nums: p.nums, k: p.k, carry: p.carry}) / 10, acc: [column_total({nums: p.nums, k: p.k, carry: p.carry}) % 10] + p.acc})

fn first_ten(v: Vec<Int>) -> Vec<Int> = collect(range(10)) | map(v[.]!)

fn leading_digits(nums: Vec<Vec<Int>>) -> Vec<Int> =
    first_ten(
        add_digits(
            {nums: nums, k: length(nums[0]!) - 1, carry: 0, acc: empty()}
        )
    )

leading_digits(input)
```

```input
[[9,9,9,9,9,9,9,9,9,9],[9,9,9,9,9,9,9,9,9,9],[0,0,0,0,0,0,0,0,0,2]]
```

```output
[2,0,0,0,0,0,0,0,0,0]
```
