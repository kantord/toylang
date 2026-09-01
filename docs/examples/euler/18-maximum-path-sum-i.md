# The richest way down a triangle

Solves [Project Euler 18](https://projecteuler.net/problem=18). See the
[spoiler warning](00-spoiler-warning.md).

The 15-row triangle is problem-given data
([kantord/toylang#39](https://github.com/kantord/toylang/issues/39)); the fragment below runs on a
synthetic 4-row triangle and the real-sized check lives in `tests/euler_real_data.rs`, opt-in via
`just euler-data DIR`.

The approach folds from the bottom up: `merge_row` replaces each entry of a row with that entry
plus the larger of the two below it, and `collapse` repeats that until row 0 holds one number.
Branching top-down instead would revisit the same cell many times over. The example's best path
is 5 -> 8 -> 9 -> 6, summing to 28.

```toylang
fn combine(p: {row: Vec<Int>, below: Vec<Int>, i: Int}) -> Int =
    p.row[p.i]! +
        (
            p
                | .below[.i]! > .below[.i + 1]! -> p.below[p.i]! or
                      p.below[p.i + 1]!
        )

fn merge_row(p: {row: Vec<Int>, below: Vec<Int>}) -> Vec<Int> =
    collect(range(length(p.row)))
        | map(combine({row: p.row, below: p.below, i: .}))

fn collapse(p: {rows: Vec<Vec<Int>>, i: Int, acc: Vec<Int>}) -> Int =
    p
        | .i < 0 -> p.acc[0]! or
              collapse({rows: p.rows, i: p.i - 1, acc: merge_row({row: p.rows[p.i]!, below: p.acc})})

fn triangle_max(rows: Vec<Vec<Int>>) -> Int =
    collapse({rows: rows, i: length(rows) - 2, acc: rows[length(rows) - 1]!})

triangle_max(input)
```

```input
[[5],[8,3],[9,1,2],[6,4,7,1]]
```

```output
28
```
