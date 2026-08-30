# How many powers are truly distinct

Solves [Project Euler 29](https://projecteuler.net/problem=29). See the
[spoiler warning](00-spoiler-warning.md).

`a^b` for `a` up to 100 and `b` up to 100 overflows past any fixed-width integer almost
immediately -- `2^100` alone is nowhere near `Int64`'s reach, let alone `Int`'s -- so nothing
here ever computes a power. Two numbers `a^b` and `c^d` coincide only when `a` and `c` are
powers of a common base, so every `a` is rewritten as `root^mult` with `root` chosen as small
as possible (`find_root_for_mult` tries the widest exponent first and works down); `a^b` then
stands for the pair `(root, mult * b)`, and counting distinct pairs counts distinct powers
without ever forming one. `root <= 100` and `mult * b <= 600` pack losslessly into one `Int`
key (`root * 1000 + mult * b`), which is where this stops being a `BigInt` problem
([contrast problem 25](25-1000-digit-fibonacci-number.md), which has no such trick available).

That still leaves counting *distinct* keys among the 9801 `(a, b)` pairs, and the first
attempt at it -- compare every key against every earlier one, roughly 48 million comparisons
-- ran fine on the compiled backends but did not return from jq inside two minutes: a real
data point for [kantord/toylang#86](https://github.com/kantord/toylang/issues/86)'s missing
`Vec` sort, the same gap [problem 22](22-names-scores.md) hits directly. Keys can only collide
when their `root` matches, though, and only six roots in `[2, 100]` (`2, 3, 5, 6, 7, 10`) have
more than one power in range at all -- every other `a` is already the smallest thing it's a
power of, so its 99 values of `b` are 99 guaranteed-distinct keys with nothing to check
against. Restricting the pairwise comparison to those six small buckets (594 pairs at most,
for root 2's six powers) cuts the 48 million comparisons to a few hundred thousand, and jq
comes back in under a second.

```toylang
fn ipow(p: {r: Int, m: Int}) -> Int =
    1 if p.m == 0 else p.r * ipow({r: p.r, m: p.m - 1})

fn find_root_for_mult(p: {a: Int, m: Int, r: Int}) -> Int =
    -1 if ipow({r: p.r, m: p.m}) > p.a else
        p.r if ipow({r: p.r, m: p.m}) == p.a else
        find_root_for_mult({a: p.a, m: p.m, r: p.r + 1})

fn best_mult(p: {a: Int, m: Int}) -> {root: Int, mult: Int} =
    {root: p.a, mult: 1} if p.m == 0 else
        {root: find_root_for_mult({a: p.a, m: p.m, r: 2}), mult: p.m} if find_root_for_mult({a: p.a, m: p.m, r: 2}) != -1 else
        best_mult({a: p.a, m: p.m - 1})

fn root_and_mult(a: Int) -> {root: Int, mult: Int} = best_mult({a: a, m: 6})

fn is_primitive(a: Int) -> Bool = root_and_mult(a).mult == 1

fn is_dup(p: {v: Vec<Int>, i: Int}) -> Bool =
    length(collect(range(p.i)) | select(p.v[.]! == p.v[p.i]!)) > 0

fn distinct_count(v: Vec<Int>) -> Int =
    length(collect(range(length(v))) | select(not is_dup({v: v, i: .})))

fn powers_from(p: {r: Int, val: Int, j: Int, top: Int}) -> Vec<Int> =
    [] if p.val > p.top else
        [p.j] + powers_from({r: p.r, val: p.val * p.r, j: p.j + 1, top: p.top})

fn powers_of(p: {r: Int, top: Int}) -> Vec<Int> =
    powers_from({r: p.r, val: p.r, j: 1, top: p.top})

fn row_for_j(p: {j: Int, top: Int}) -> Vec<Int> =
    collect(range(p.top - 1)) | map(. + 2) | map(p.j * .)

fn exponents_for_root(p: {r: Int, top: Int}) -> Vec<Int> =
    flatten(
        powers_of({r: p.r, top: p.top}) | map(row_for_j({j: ., top: p.top}))
    )

fn multi_root_contribution(p: {r: Int, top: Int}) -> Int =
    distinct_count(exponents_for_root(p))

fn single_contrib(top: Int) -> Int =
    length(
        collect(range(top - 1))
            | map(. + 2)
            | select(is_primitive(.) and . * . > top)
    ) *
        (top - 1)

fn multi_contribs(top: Int) -> Vec<Int> =
    collect(range(top - 1))
        | map(. + 2)
        | select(. * . <= top and is_primitive(.))
        | map(multi_root_contribution({r: ., top: top}))

fn sum_ints(v: Vec<Int>) -> Int =
    0 if length(v) == 0 else v[0]! + sum_ints(tail(v)!)

fn solve(top: Int) -> Int = single_contrib(top) + sum_ints(multi_contribs(top))

solve(100)
```

```output
9183
```
