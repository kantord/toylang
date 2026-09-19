# The unit fraction with the longest repeating cycle

Solves [Project Euler 26](https://projecteuler.net/problem=26). See the
[spoiler warning](00-spoiler-warning.md).

`1/d`'s repeating cycle length is the smallest `k` with `10^k mod d == 1`, once the factors of
2 and 5 are stripped out of `d` (they only lengthen the non-repeating prefix, never the cycle
itself). `walk` finds `k` by long division one digit at a time, carrying the remainder forward
until it comes back to 1: up to `d - 1` steps, each a self-tail-call, so the walk runs in
constant stack on every backend
([kantord/toylang#141](https://github.com/kantord/toylang/issues/141)). `find_best` keeps the
`{d, len}` pair with the longest cycle by halving the range and comparing the two halves'
winners with `best_of`; [`max`](../../reference/builtins/max.md) is defined only for a `Vec`
of integers, and `max_by`, which would pick the pair with the largest `.len` directly, is not
yet on every backend.

```toylang
fn strip2(n: Int) -> Int = n | . % 2 == 0 -> strip2(. / 2) or .

fn strip5(n: Int) -> Int = n | . % 5 == 0 -> strip5(. / 5) or .

fn reduced(d: Int) -> Int = strip5(strip2 d)

fn walk({ m, r, count }: { m: Int, r: Int, count: Int }) -> Int =
  r
  | . == 1 -> count or walk { m: m, r: r * 10 % m, count: count + 1 }

fn cycle_length(d: Int) -> Int =
  let m = reduced d
  m | . == 1 -> 0 or walk { m: m, r: 10 % m, count: 1 }

fn best_of(
  { a, b }: { a: { d: Int, len: Int }, b: { d: Int, len: Int } }
) -> { d: Int, len: Int } =
  a | a.len >= b.len -> a or b

fn find_best(
  { lo, hi }: { lo: Int, hi: Int }
) -> { d: Int, len: Int } =
  hi - lo
  | . == 1 -> { d: lo, len: cycle_length lo } or
    best_of {
      a: find_best { lo: lo, hi: (lo + hi) / 2 },
      b: find_best { lo: (lo + hi) / 2, hi: hi }
    }

find_best({ lo: 2, hi: 1000 }).d
```

```output
983
```
