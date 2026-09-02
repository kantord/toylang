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
fn strip2(n: Int) -> Int = n | . % 2 == 0 -> strip2(. / 2) or .

fn strip5(n: Int) -> Int = n | . % 5 == 0 -> strip5(. / 5) or .

fn reduced(d: Int) -> Int = strip5(strip2(d))

fn cycle_inner(p: {m: Int, r: Int, count: Int, steps_left: Int}) -> {done: Int, count: Int, r: Int} =
    p
        | .r == 1 -> {done: 1, count: .count, r: .r} or
              .steps_left == 0 -> {done: 0, count: .count, r: .r} or
              cycle_inner({m: .m, r: .r * 10 % .m, count: .count + 1, steps_left: .steps_left - 1})

fn cycle_outer(p: {m: Int, r: Int, count: Int, chunks_left: Int}) -> Int =
    p
        | .chunks_left == 0 -> -1 or
              cycle_inner({m: .m, r: .r, count: .count, steps_left: 100}).done == 1 -> cycle_inner({m: .m, r: .r, count: .count, steps_left: 100}).count or
              cycle_outer({m: .m, r: cycle_inner({m: .m, r: .r, count: .count, steps_left: 100}).r, count: cycle_inner({m: .m, r: .r, count: .count, steps_left: 100}).count, chunks_left: .chunks_left - 1})

fn cycle_length(d: Int) -> Int =
    d
        | reduced(.) == 1 -> 0 or
              cycle_outer({m: reduced(.), r: 10 % reduced(.), count: 1, chunks_left: 10})

fn best_of(p: {a: {d: Int, len: Int}, b: {d: Int, len: Int}}) -> {d: Int, len: Int} =
    p | .a.len >= .b.len -> .a or .b

fn find_best(p: {lo: Int, hi: Int}) -> {d: Int, len: Int} =
    p
        | .hi - .lo == 1 -> {d: .lo, len: cycle_length(.lo)} or
              best_of({a: find_best({lo: .lo, hi: (.lo + .hi) / 2}), b: find_best({lo: (.lo + .hi) / 2, hi: .hi})})

find_best({lo: 2, hi: 1000}).d
```

```output
983
```
