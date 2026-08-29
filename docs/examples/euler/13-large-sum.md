# Summing a hundred large numbers

Solves [Project Euler 13](https://projecteuler.net/problem=13). See the
[spoiler warning](00-spoiler-warning.md).

The hundred 50-digit numbers are [problem-given data with no source but Project Euler
itself](https://github.com/kantord/toylang/issues/39), so they never appear in this page or the
repo: the program reads them from stdin, gitignored locally as
`docs/examples/euler/fixtures/13.json`, a JSON array of a hundred arrays of fifty digits each,
most-significant digit first. Anyone without that file gets the program and the claim, not a
way to check it -- the docs harness skips this fragment rather than failing when the file
isn't there.

Neither the input nor the sum fits `Int`, which is 32 bits
([kantord/toylang#38](https://github.com/kantord/toylang/issues/38)): that is why the fixture
stores each number as its own digits rather than one JSON integer, which `input` would refuse
outright as not fitting the type. The sum itself is done the way it would be on paper, one
column of digits at a time
from the right, carrying into the next column; every column total (at most a hundred nines plus
a small carry) stays far inside `Int`, even though the sum as a whole does not. Only the
leading ten digits the problem asks for are kept, so the answer is a `Vec<Int>` of digits
rather than a number that would not fit either.

```toylang
fn empty() -> Vec<Int> = []

fn col_sum(p: {nums: Vec<Vec<Int>>, i: Int, k: Int}) -> Int =
    0 if p.i >= extent(p.nums) else
    p.nums[p.i]![p.k]! + col_sum({nums: p.nums, i: p.i + 1, k: p.k})

fn column_total(p: {nums: Vec<Vec<Int>>, k: Int, carry: Int}) -> Int =
    col_sum({nums: p.nums, i: 0, k: p.k}) + p.carry

fn emit_carry(p: {carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    p.acc if p.carry == 0 else
    emit_carry({carry: p.carry / 10, acc: concat([[p.carry % 10], p.acc])})

fn add_digits(p: {nums: Vec<Vec<Int>>, k: Int, carry: Int, acc: Vec<Int>}) -> Vec<Int> =
    emit_carry({carry: p.carry, acc: p.acc}) if p.k < 0 else
    add_digits({
        nums: p.nums,
        k: p.k - 1,
        carry: column_total({nums: p.nums, k: p.k, carry: p.carry}) / 10,
        acc: concat([[column_total({nums: p.nums, k: p.k, carry: p.carry}) % 10], p.acc])
    })

fn first_ten(v: Vec<Int>) -> Vec<Int> = range(10) | map(v[.]!)

fn leading_digits(nums: Vec<Vec<Int>>) -> Vec<Int> =
    first_ten(add_digits({nums: nums, k: extent(nums[0]!) - 1, carry: 0, acc: empty()}))

leading_digits(input)
```

```fixture
docs/examples/euler/fixtures/13.json
```

```output
[5,5,3,7,3,7,6,2,3,0]
```

The output above is the answer as commonly published for this problem; nothing in this repo
has run the program against the real hundred numbers; there is nowhere here for them to live.
The column-addition logic itself is checked against synthetic batches of numbers instead,
including a batch of all nines to force the carry as wide as it gets.
