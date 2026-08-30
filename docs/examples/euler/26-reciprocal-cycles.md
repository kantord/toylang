# The unit fraction with the longest repeating cycle

Solves [Project Euler 26](https://projecteuler.net/problem=26). See the
[spoiler warning](00-spoiler-warning.md).

`1/d`'s repeating cycle length is the smallest `k` with `10^k mod d == 1`, once the factors of
2 and 5 are stripped out of `d` (they only lengthen the non-repeating prefix, never the cycle
itself). Walking that remainder forward one digit of long division at a time is the direct
way to find `k`, but a plain recursive walk could run up to `d - 1` steps deep -- as many as
998 frames for `d` just under 1000, the same ceiling [problem 17](17-number-letter-counts.md)
and [problem 19](19-counting-sundays.md) worry about. `cycle_inner` walks a fixed chunk of a
hundred digits and reports where it stopped; `cycle_outer` calls it chunk after chunk,
carrying the remainder forward, so no single call frame goes deeper than about a hundred.

```toylang
fn strip2(n: Int) -> Int = strip2(n / 2) if n % 2 == 0 else n

fn strip5(n: Int) -> Int = strip5(n / 5) if n % 5 == 0 else n

fn reduced(d: Int) -> Int = strip5(strip2(d))

fn cycle_inner(p: {m: Int, r: Int, count: Int, steps_left: Int}) -> {done: Int, count: Int, r: Int} =
    {done: 1, count: p.count, r: p.r} if p.r == 1 else
        {done: 0, count: p.count, r: p.r} if p.steps_left == 0 else
        cycle_inner(
            {
                m: p.m,
                r: p.r * 10 % p.m,
                count: p.count + 1,
                steps_left: p.steps_left - 1
            }
        )

fn cycle_outer(p: {m: Int, r: Int, count: Int, chunks_left: Int}) -> Int =
    -1 if p.chunks_left == 0 else
        cycle_inner({m: p.m, r: p.r, count: p.count, steps_left: 100}).count if cycle_inner({m: p.m, r: p.r, count: p.count, steps_left: 100}).done == 1 else
        cycle_outer(
            {
                m: p.m,
                r: cycle_inner({m: p.m, r: p.r, count: p.count, steps_left: 100}).r,
                count: cycle_inner({m: p.m, r: p.r, count: p.count, steps_left: 100}).count,
                chunks_left: p.chunks_left - 1
            }
        )

fn cycle_length(d: Int) -> Int =
    0 if reduced(d) == 1 else
        cycle_outer(
            {m: reduced(d), r: 10 % reduced(d), count: 1, chunks_left: 10}
        )

fn best_of(p: {a: {d: Int, len: Int}, b: {d: Int, len: Int}}) -> {d: Int, len: Int} =
    p.a if p.a.len >= p.b.len else p.b

fn find_best(p: {lo: Int, hi: Int}) -> {d: Int, len: Int} =
    {d: p.lo, len: cycle_length(p.lo)} if p.hi - p.lo == 1 else
        best_of(
            {
                a: find_best({lo: p.lo, hi: (p.lo + p.hi) / 2}),
                b: find_best({lo: (p.lo + p.hi) / 2, hi: p.hi})
            }
        )

find_best({lo: 2, hi: 1000}).d
```

```output
983
```
