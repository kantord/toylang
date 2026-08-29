# Four in a row, multiplied

Solves [Project Euler 11](https://projecteuler.net/problem=11). See the
[spoiler warning](00-spoiler-warning.md).

The 20x20 grid is [problem-given data with no source but Project Euler
itself](https://github.com/kantord/toylang/issues/39), so it never appears in this page or the
repo: the program reads it from stdin, gitignored locally as `docs/examples/euler/fixtures/11.json`,
a JSON array of 20 rows of 20 integers each, row-major, matching the grid as printed. Anyone
without that file gets the program and the claim, not a way to check it -- the docs harness
skips this fragment rather than failing when the file isn't there.

`four` multiplies the entry at `(r, c)` and the next three steps of `(dr, dc)`; `direction`
walks every valid starting cell for one of the four directions (right, down, and both
diagonals) and collects the products, bounding each loop so no step reaches outside the grid
rather than defaulting an out-of-range read to zero. `maximum` is the same recursive
running-best pattern as [problem 12's divisor count](12-highly-divisible-triangular-number.md),
generalized to a whole `Vec` since there is no `max` builtin.

```toylang
fn get(p: {g: Vec<Vec<Int>>, r: Int, c: Int}) -> Int = p.g[p.r]![p.c]!

fn four(p: {g: Vec<Vec<Int>>, r: Int, c: Int, dr: Int, dc: Int}) -> Int =
    get({g: p.g, r: p.r, c: p.c}) * get({g: p.g, r: p.r + p.dr, c: p.c + p.dc}) *
        get({g: p.g, r: p.r + 2 * p.dr, c: p.c + 2 * p.dc}) *
        get({g: p.g, r: p.r + 3 * p.dr, c: p.c + 3 * p.dc})

fn row_products(p: {g: Vec<Vec<Int>>, r: Int, dr: Int, dc: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    range(p.cmax) | select(. >= p.cmin) |
        map(four({g: p.g, r: p.r, c: ., dr: p.dr, dc: p.dc}))

fn direction(p: {g: Vec<Vec<Int>>, dr: Int, dc: Int, rmax: Int, cmin: Int, cmax: Int}) -> Vec<Int> =
    concat(
        range(p.rmax) |
            map(
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
    p.best if p.i >= extent(p.v) else
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
        concat(
            [
                direction(
                    {
                        g: g,
                        dr: 0,
                        dc: 1,
                        rmax: extent(g),
                        cmin: 0,
                        cmax: extent(g[0]!) - 3
                    }
                ),
                direction(
                    {
                        g: g,
                        dr: 1,
                        dc: 0,
                        rmax: extent(g) - 3,
                        cmin: 0,
                        cmax: extent(g[0]!)
                    }
                ),
                direction(
                    {
                        g: g,
                        dr: 1,
                        dc: 1,
                        rmax: extent(g) - 3,
                        cmin: 0,
                        cmax: extent(g[0]!) - 3
                    }
                ),
                direction(
                    {
                        g: g,
                        dr: 1,
                        dc: -1,
                        rmax: extent(g) - 3,
                        cmin: 3,
                        cmax: extent(g[0]!)
                    }
                )
            ]
        )
    )

largest_product(input)
```

```fixture
docs/examples/euler/fixtures/11.json
```

```output
70600674
```

The output above is the answer as commonly published for this problem; nothing in this repo
has run the program against the real grid; there is nowhere here for the real grid to live.
The logic itself is checked against synthetic grids of various sizes instead, the same way any
other function in this codebase is tested against cases that do not depend on Project Euler's
data.
