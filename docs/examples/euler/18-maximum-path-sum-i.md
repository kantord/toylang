# The richest way down a triangle

Solves [Project Euler 18](https://projecteuler.net/problem=18). See the
[spoiler warning](00-spoiler-warning.md).

The 15-row triangle is [problem-given data with no source but Project Euler
itself](https://github.com/kantord/toylang/issues/39), so it never appears in this page or the
repo: the program reads it from stdin, gitignored locally as `docs/examples/euler/fixtures/18.json`,
a JSON array of 15 rows, row `i` holding `i + 1` integers, top to bottom as printed. Anyone
without that file gets the program and the claim, not a way to check it -- the docs harness
skips this fragment rather than failing when the file isn't there.

Working top-down and branching at every row would revisit the same cell many times over; this
instead folds from the bottom up, the standard trick for this problem. `merge_row` replaces a
row with, at each position, that entry plus the larger of the two entries below it reachable
from there; folded all the way to the top, row 0 is left holding one number, the best total.
`collapse` is the fold, walking `rows` from the second-to-last back to the first and carrying
the row built so far as its accumulator, the same recursive-accumulator shape as [problem 2's
Fibonacci sum](02-even-fibonacci-sum.md).

```toylang
fn combine(p: {row: Vec<Int>, below: Vec<Int>, i: Int}) -> Int =
    p.row[p.i]! + (p.below[p.i]! if p.below[p.i]! > p.below[p.i + 1]! else p.below[p.i + 1]!)

fn merge_row(p: {row: Vec<Int>, below: Vec<Int>}) -> Vec<Int> =
    range(extent(p.row)) | map(combine({row: p.row, below: p.below, i: .}))

fn collapse(p: {rows: Vec<Vec<Int>>, i: Int, acc: Vec<Int>}) -> Int =
    p.acc[0]! if p.i < 0 else
    collapse({rows: p.rows, i: p.i - 1, acc: merge_row({row: p.rows[p.i]!, below: p.acc})})

fn triangle_max(rows: Vec<Vec<Int>>) -> Int =
    collapse({rows: rows, i: extent(rows) - 2, acc: rows[extent(rows) - 1]!})

triangle_max(input)
```

```fixture
docs/examples/euler/fixtures/18.json
```

```output
1074
```

The output above is the answer as commonly published for this problem; nothing in this repo
has run the program against the real triangle; there is nowhere here for the real triangle to
live. The logic itself is checked against synthetic triangles instead, small ones by hand and
larger ones against a reference bottom-up fold.
