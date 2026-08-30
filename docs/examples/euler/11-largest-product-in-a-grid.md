# Four in a row, multiplied

Solves [Project Euler 11](https://projecteuler.net/problem=11). See the
[spoiler warning](00-spoiler-warning.md).

The 20x20 grid is problem-given data, so
[kantord/toylang#39](https://github.com/kantord/toylang/issues/39) keeps it out of this repo; the
fragment below runs on a synthetic 4x4 grid and the real-sized check lives in
`tests/euler_real_data.rs`, opt-in via `just euler-data DIR`.

Each of the four directions is a `(dr, dc)` step, and `row_products` bounds the starting column
so no step reaches outside the grid. Every product each direction can form is collected, the four
`Vec`s are concatenated, and `maximum_of` walks the flattened list once for the largest -- there
is no `max` fold. The example grid is 1 to 16 in order, where the bottom row's 13*14*15*16 =
43680 wins only because a row alone runs that far; the point of checking all four directions is
that the real answer usually comes from a diagonal.

```toylang
fn get(p: {g: Vec<Vec<Int>>, r: Int, c: Int}) -> Int = p.g[p.r]![p.c]!

fn four(p: {g: Vec<Vec<Int>>, r: Int, c: Int, dr: Int, dc: Int}) -> Int =
    get({g: p.g, r: p.r, c: p.c}) * get({g: p.g, r: p.r + p.dr, c: p.c + p.dc}) *
        get({g: p.g, r: p.r + 2 * p.dr, c: p.c + 2 * p.dc}) *
        get({g: p.g, r: p.r + 3 * p.dr, c: p.c + 3 * p.dc})

fn row_products(p: {g: Vec<Vec<Int>>, r: Int, dr: Int, dc: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    range(p.cmax)
        | select(. >= p.cmin)
        | map(four({g: p.g, r: p.r, c: ., dr: p.dr, dc: p.dc}))

fn direction(p: {g: Vec<Vec<Int>>, dr: Int, dc: Int, rmax: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    flatten(
        range(p.rmax)
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

fn maximum_of(p: {v: Vec<Int>, i: Int, best: Int}) -> Int =
    p.best if p.i >= length(p.v) else
        maximum_of(
            {
                v: p.v,
                i: p.i + 1,
                best: p.v[p.i]! if p.v[p.i]! > p.best else p.best
            }
        )

fn maximum(v: Vec<Int>) -> Int = maximum_of({v: v, i: 1, best: v[0]!})

fn largest_product(g: Vec<Vec<Int>>) -> Int =
    maximum(
        direction(
            {
                g: g,
                dr: 0,
                dc: 1,
                rmax: length(g),
                cmin: 0,
                cmax: length(g[0]!) - 3
            }
        ) +
            direction(
                {
                    g: g,
                    dr: 1,
                    dc: 0,
                    rmax: length(g) - 3,
                    cmin: 0,
                    cmax: length(g[0]!)
                }
            ) +
            direction(
                {
                    g: g,
                    dr: 1,
                    dc: 1,
                    rmax: length(g) - 3,
                    cmin: 0,
                    cmax: length(g[0]!) - 3
                }
            ) +
            direction(
                {
                    g: g,
                    dr: 1,
                    dc: -1,
                    rmax: length(g) - 3,
                    cmin: 3,
                    cmax: length(g[0]!)
                }
            )
    )

largest_product(input)
```

```input
[[1,2,3,4],[5,6,7,8],[9,10,11,12],[13,14,15,16]]
```

```output
43680
```

At real size this program stalls on one backend: `maximum_of`'s linear scan over the flattened
~1258 products blows past the Python backend's 1000-frame recursion limit
([kantord/toylang#132](https://github.com/kantord/toylang/issues/132)), a gap in `emit_py.rs`,
not in the language. At this 4x4 scale the scan is ten frames, and every backend agrees.
