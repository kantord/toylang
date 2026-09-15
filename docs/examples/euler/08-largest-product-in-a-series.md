# The best window of digits

Solves [Project Euler 8](https://projecteuler.net/problem=8). See the
[spoiler warning](00-spoiler-warning.md).

The thousand digits are problem-given data, so
[kantord/toylang#39](https://github.com/kantord/toylang/issues/39) keeps them out of this repo and
the fragment below checks the program on a synthetic fourteen-digit number instead. The
real-sized check lives in `tests/euler_real_data.rs`, opt-in: `just euler-data DIR` runs this
exact program against your own copy of the thousand digits and fails loudly on a wrong answer.

`window` multiplies the `k` digits starting at `i`, and `windows` maps every window start from 0
to `length(input) - 13` to its product, so each start's product is computed once; `max` picks
the winner in one call. The example is the smallest interesting shape: a 1 followed by thirteen
9s, whose two windows are `1*9^12` and `9^13`. The winner is why this page waited on
[Int64](../../reference/types/int64.md): `9^13` is about 2.5e12, past `Int`'s 32-bit ceiling.

```toylang
fn window(p: {v: Vec<Int>, i: Int, k: Int}) -> Int64 =
    p
        | .k == 0 -> 1 or
              i64(p.v[p.i + p.k - 1]!) * window({v: p.v, i: p.i, k: p.k - 1})

fn windows(v: Vec<Int>) -> Vec<Int64> =
    collect(range(length(v) - 12)) | map(window({v: v, i: ., k: 13}))

max(windows(parse(stdin)))!
```

```input
[1,9,9,9,9,9,9,9,9,9,9,9,9,9]
```

```output
2541865828329
```
