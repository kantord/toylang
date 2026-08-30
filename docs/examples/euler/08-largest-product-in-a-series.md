# The best window of digits

Solves [Project Euler 8](https://projecteuler.net/problem=8). See the
[spoiler warning](00-spoiler-warning.md).

Both of this page's old blockers are gone. The problem's thousand digits are problem-given
data, so they arrive on stdin under [the fixture protocol #39
settled](https://github.com/kantord/toylang/issues/39): gitignored locally as
`docs/examples/euler/fixtures/08.json`, a JSON array of the thousand digits in order, and the
docs harness skips the fragment rather than failing when the file isn't there. And the
largest product of thirteen digits -- `9^13`, about 2.5e12 -- now has a type that holds it:
[Int64](../../reference/types/int64.md)
([kantord/toylang#83](https://github.com/kantord/toylang/issues/83)). The digits themselves
stay `Int`; each one is widened by `i64(...)` as it enters the product.

One window's product is a thirteen-deep recursion. The maximum over all 988 windows splits
the range in half and recurses instead of walking it linearly: a fold-shaped recursion 988
deep would sit close to Python's default recursion limit, and about ten levels of halving
plus thirteen of product stays shallow everywhere.

```toylang
fn window(p: {v: Vec<Int>, i: Int, k: Int}) -> Int64 =
    1 if p.k == 0 else
        i64(p.v[p.i + p.k - 1]!) * window({v: p.v, i: p.i, k: p.k - 1})

fn max2(p: {a: Int64, b: Int64}) -> Int64 = p.a if p.a > p.b else p.b

fn best(p: {v: Vec<Int>, lo: Int, hi: Int}) -> Int64 =
    window({v: p.v, i: p.lo, k: 13}) if p.hi - p.lo == 1 else
        max2(
            {
                a: best({v: p.v, lo: p.lo, hi: (p.lo + p.hi) / 2}),
                b: best({v: p.v, lo: (p.lo + p.hi) / 2, hi: p.hi})
            }
        )

best({v: input, lo: 0, hi: extent(input) - 12})
```

```fixture
docs/examples/euler/fixtures/08.json
```

```output
23514624000
```

The output above is the answer as commonly published for this problem, the same standing
[problem 13](13-large-sum.md)'s has: the thousand digits have nowhere to live in this repo,
so the harness only checks the claim when a contributor supplies their own copy. Every
product stays under 2.6e12, inside the jq backend's
[documented Int64 precision envelope](../../reference/types/int64.md), so all seven backends
agree exactly when it runs.
