# The unit fraction with the longest repeating cycle

Solves [Project Euler 26](https://projecteuler.net/problem=26). See the
[spoiler warning](00-spoiler-warning.md).

`1/d`'s repeating cycle length is the smallest `k` with `10^k mod d == 1`, once the factors of
2 and 5 are stripped out of `d` (they only lengthen the non-repeating prefix, never the cycle
itself). `walk` finds `k` by long division one digit at a time, carrying the remainder forward
until it comes back to 1: up to `d - 1` steps, each a self-tail-call, so the walk runs in
constant stack on every backend
([kantord/toylang#141](https://github.com/kantord/toylang/issues/141)). The pipeline pairs each
`d` from 2 to 999 with its cycle length and takes
[`max_by(.len)`](../../reference/builtins/max_by.md), so the answer is the `d` of the pair with
the longest cycle. [`max`](../../reference/builtins/max.md) would reduce the lengths alone and
lose which `d` they belong to. `max_by` returns an `Opt`, unwrapped with `!`, and keeps the
first of equal maxima.

```toylang
fn strip2(n: Int) -> Int = n | . % 2 == 0 -> strip2(. / 2) or .;


fn strip5(n: Int) -> Int = n | . % 5 == 0 -> strip5(. / 5) or .;


fn reduced(d: Int) -> Int = strip5(strip2 d);


fn walk({ m, r, count }: { m: Int, r: Int, count: Int }) -> Int =
  r
  | . == 1 -> count or walk { m: m, r: r * 10 % m, count: count + 1 };


fn cycle_length(d: Int) -> Int =
  let m = reduced d

  m | . == 1 -> 0 or walk { m: m, r: 10 % m, count: 1 };


collect(range 1000)
| select(. >= 2)
| map { d: ., len: cycle_length(.) }
| max_by(.len)!.d
```

```output
983
```
