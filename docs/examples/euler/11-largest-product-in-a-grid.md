# Four in a row, multiplied

Solves [Project Euler 11](https://projecteuler.net/problem=11). See the
[spoiler warning](00-spoiler-warning.md).

The 20x20 grid is problem-given data, so
[kantord/toylang#39](https://github.com/kantord/toylang/issues/39) keeps it out of this repo; the
fragment below runs on a synthetic 4x4 grid and the real-sized check lives in
`tests/euler_real_data.rs`, opt-in via `just euler-data DIR`.

Each of the four directions is a `(dr, dc)` step, and `row_products` bounds the starting column
so no step reaches outside the grid. Every product each direction can form is collected, the four
`Vec`s are flattened into one list, and `max` reduces it directly. The example grid is 1
to 16 in order, where the bottom row's 13*14*15*16 = 43680 wins only because a row alone runs
that far; the point of checking all four directions is that the real answer usually comes from a
diagonal.

```toylang
fn get({ g, r, c }: { g: Vec<Vec<Int>>, r: Int, c: Int }) -> Int =
  g[r]![c]!

fn four(
  { g, r, c, dr, dc }: {
    g: Vec<Vec<Int>>,
    r: Int,
    c: Int,
    dr: Int,
    dc: Int
  }
) -> Int =
  get({ g: g, r: r, c: c }) * get({ g: g, r: r + dr, c: c + dc }) *
    get({ g: g, r: r + 2 * dr, c: c + 2 * dc }) *
    get({ g: g, r: r + 3 * dr, c: c + 3 * dc })

fn row_products(
  { g, r, dr, dc, cmin, cmax }: {
    g: Vec<Vec<Int>>,
    r: Int,
    dr: Int,
    dc: Int,
    cmin: Int,
    cmax: Int
  }
) -> Vec<Int> =
  collect(range(cmax))
  | select(. >= cmin)
  | map(four({ g: g, r: r, c: ., dr: dr, dc: dc }))

fn direction(
  { g, dr, dc, rmax, cmin, cmax }: {
    g: Vec<Vec<Int>>,
    dr: Int,
    dc: Int,
    rmax: Int,
    cmin: Int,
    cmax: Int
  }
) -> Vec<Int> =
  flatten(
    collect(range(rmax))
    | map(
        row_products(
          { g: g, r: ., dr: dr, dc: dc, cmin: cmin, cmax: cmax }
        )
      )
  )

fn largest_product(g: Vec<Vec<Int>>) -> Int =
  let rows = length(g)
  let cols = length(g[0]!)
  let right = direction({ g: g, dr: 0, dc: 1, rmax: rows, cmin: 0, cmax: cols - 3 })
  let down = direction({ g: g, dr: 1, dc: 0, rmax: rows - 3, cmin: 0, cmax: cols })
  let diagonal = direction({ g: g, dr: 1, dc: 1, rmax: rows - 3, cmin: 0, cmax: cols - 3 })
  let antidiagonal = direction({ g: g, dr: 1, dc: -1, rmax: rows - 3, cmin: 3, cmax: cols })
  max(flatten([right, down, diagonal, antidiagonal]))!

largest_product(parse(stdin))
```

```input
[[1,2,3,4],[5,6,7,8],[9,10,11,12],[13,14,15,16]]
```

```output
43680
```
