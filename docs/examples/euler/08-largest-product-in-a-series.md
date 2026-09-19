# The best window of digits

Solves [Project Euler 8](https://projecteuler.net/problem=8). See the
[spoiler warning](00-spoiler-warning.md).

The thousand digits are problem-given data, so
[kantord/toylang#39](https://github.com/kantord/toylang/issues/39) keeps them out of this repo and
the fragment below checks the program on a synthetic fourteen-digit number instead. The
real-sized check lives in `tests/euler_real_data.rs`, opt-in: `just euler-data DIR` runs this
exact program against your own copy of the thousand digits and fails loudly on a wrong answer.

`product` multiplies a window's digits, and `windows` slices every thirteen-digit window out
of the input (`v[i:i + 13]`) and maps it to its product, so each window is multiplied once;
`max` picks the winner in one call. The example is the smallest interesting shape: a 1
followed by thirteen 9s, whose two windows are `1*9^12` and `9^13`. The winner is why the
product is an [Int64](../../reference/types/int64.md): `9^13` is about 2.5e12, past `Int`'s
32-bit ceiling.

```toylang
fn product(v: Vec<Int>) -> Int64 =
  length(v) == 0 | . -> 1 or i64(v[0]!) * product(tail(v)!)


fn windows(v: Vec<Int>) -> Vec<Int64> =
  collect range(length(v) - 12) | map(product v[.:. + 13])


max(windows(parse stdin))!
```

```input
[1,9,9,9,9,9,9,9,9,9,9,9,9,9]
```

```output
2541865828329
```
