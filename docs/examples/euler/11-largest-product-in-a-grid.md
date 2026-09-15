# Four in a row, multiplied

Solves [Project Euler 11](https://projecteuler.net/problem=11). See the
[spoiler warning](00-spoiler-warning.md).

The 20x20 grid is problem-given data, so
[kantord/toylang#39](https://github.com/kantord/toylang/issues/39) keeps it out of this repo; the
fragment below runs on a synthetic 4x4 grid and the real-sized check lives in
`tests/euler_real_data.rs`, opt-in via `just euler-data DIR`.

Each of the four directions is a `(dr, dc)` step, and `row_products` bounds the starting column
so no step reaches outside the grid. Every product each direction can form is collected, the four
`Vec`s are concatenated, and `max` reduces the concatenated list directly. The example grid is 1
to 16 in order, where the bottom row's 13*14*15*16 = 43680 wins only because a row alone runs
that far; the point of checking all four directions is that the real answer usually comes from a
diagonal.

```toylang
fn get(p: {g: Vec<Vec<Int>>, r: Int, c: Int}) -> Int = p.g[p.r]![p.c]!

fn four(p: {g: Vec<Vec<Int>>, r: Int, c: Int, dr: Int, dc: Int}) -> Int =
    get({g: p.g, r: p.r, c: p.c}) * get({g: p.g, r: p.r + p.dr, c: p.c + p.dc}) *
        get({g: p.g, r: p.r + 2 * p.dr, c: p.c + 2 * p.dc}) *
        get({g: p.g, r: p.r + 3 * p.dr, c: p.c + 3 * p.dc})

fn row_products(p: {g: Vec<Vec<Int>>, r: Int, dr: Int, dc: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    collect(range(p.cmax))
        | select(. >= p.cmin)
        | map(four({g: p.g, r: p.r, c: ., dr: p.dr, dc: p.dc}))

fn direction(p: {g: Vec<Vec<Int>>, dr: Int, dc: Int, rmax: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    flatten(
        collect(range(p.rmax))
            | map(
                  row_products(
                      {
                          g: p.g,
                          r: .,
                          dr: p.dr,
                          dc: p.dc,
                          cmin: p.cmin,
                          cmax: p.cmax
                      }
                  )
              )
    )

fn largest_product(g: Vec<Vec<Int>>) -> Int =
    max(direction({g: g, dr: 0, dc: 1, rmax: length(g), cmin: 0, cmax: length(g[0]!) - 3}) + direction({g: g, dr: 1, dc: 0, rmax: length(g) - 3, cmin: 0, cmax: length(g[0]!)}) + direction({g: g, dr: 1, dc: 1, rmax: length(g) - 3, cmin: 0, cmax: length(g[0]!) - 3}) + direction({g: g, dr: 1, dc: -1, rmax: length(g) - 3, cmin: 3, cmax: length(g[0]!)}))!

largest_product(parse(stdin))
```

```input
[[1,2,3,4],[5,6,7,8],[9,10,11,12],[13,14,15,16]]
```

```output
43680
```
