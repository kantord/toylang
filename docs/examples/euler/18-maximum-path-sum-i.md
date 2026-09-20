# The richest way down a triangle

Solves [Project Euler 18](https://projecteuler.net/problem=18). See the
[spoiler warning](00-spoiler-warning.md).

The 15-row triangle is problem-given data
([kantord/toylang#39](https://github.com/kantord/toylang/issues/39)); the fragment below runs on a
synthetic 4-row triangle and the real-sized check lives in `tests/euler_real_data.rs`, opt-in via
`just euler-data DIR`.

The approach folds from the bottom up: `merge_row` replaces each entry of a row with that entry
plus the larger of the two below it, `max` over the two-entry slice `below[i:i + 2]`, and
`collapse` walks the rows in reverse, feeding each the merged row beneath it, until only the apex
is left. Branching top-down instead would revisit the same cell many times over. The example's
best path is 5 -> 8 -> 9 -> 6, summing to 28.

```toylang
# fmt: syntax-example
fn merge_row(
  { row, below }: { row: Vec<Int>, below: Vec<Int> }
) -> Vec<Int> =
  collect range(length row) | map(row[.]! + max(below[.:. + 2])!);


fn collapse(
  { rows, acc }: { rows: Vec<Vec<Int>>, acc: Vec<Int> }
) -> Int =
  rows
  | length(rows) == 0 -> acc[0]! or
    collapse {
      rows: tail(rows)!,
      acc: merge_row { row: rows[0]!, below: acc }
    };


fn triangle_max(rows: Vec<Vec<Int>>) -> Int =
  collapse { rows: reverse rows[:-1], acc: rows[-1]! };


triangle_max(parse stdin)
```

```input
[[5],[8,3],[9,1,2],[6,4,7,1]]
```

```output
28
```
