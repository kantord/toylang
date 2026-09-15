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

That still leaves counting *distinct* keys among the 9801 `(a, b)` pairs, and the sort builtin
makes that a direct job: pack all 9801 keys, sort them, and count the adjacent equal pairs. A
duplicate in a sorted list always sits next to its twin, so subtracting that run count from
the total length gives the distinct count. `keys_for_a` expands one base's 99 exponents,
`all_keys` flattens those rows into the full 9801-key set, and `adjacent_dup_count` reduces
the adjacent-equal indicators with `sum`, so no key is ever compared against more than its
neighbor in the sorted order.

```toylang
fn ipow(p: {r: Int, m: Int}) -> Int =
    p | .m == 0 -> 1 or .r * ipow({r: .r, m: .m - 1})

fn find_root_for_mult(p: {a: Int, m: Int, r: Int}) -> Int =
    p
        | ipow({r: p.r, m: p.m}) > .a -> -1 or
              ipow({r: p.r, m: p.m}) == .a -> .r or
              find_root_for_mult({a: .a, m: .m, r: .r + 1})

fn best_mult(p: {a: Int, m: Int}) -> {root: Int, mult: Int} =
    p
        | .m == 0 -> {root: .a, mult: 1} or
              find_root_for_mult({a: .a, m: .m, r: 2}) != -1 -> {root: find_root_for_mult({a: .a, m: .m, r: 2}), mult: .m} or
              best_mult({a: .a, m: .m - 1})

fn root_and_mult(a: Int) -> {root: Int, mult: Int} = best_mult({a: a, m: 6})

fn key_for(p: {a: Int, b: Int}) -> Int =
    root_and_mult(p.a) | .root * 1000 + .mult * p.b

fn keys_for_a(p: {a: Int, top: Int}) -> Vec<Int> =
    collect(range(p.top - 1)) | map(. + 2) | map(key_for({a: p.a, b: .}))

fn all_keys(top: Int) -> Vec<Int> =
    flatten(
        collect(range(top - 1)) | map(. + 2) | map(keys_for_a({a: ., top: top}))
    )

fn is_adjacent_dup(p: {v: Vec<Int>, i: Int}) -> Int =
    p | p.v[p.i + 1]! == p.v[p.i]! -> 1 or 0

fn adjacent_dup_count(sorted: Vec<Int>) -> Int =
    sum(
        collect(range(length(sorted) - 1))
            | map(is_adjacent_dup({v: sorted, i: .}))
    )

fn distinct_count(v: Vec<Int>) -> Int = length(v) - adjacent_dup_count(sort(v))

fn solve(top: Int) -> Int = distinct_count(all_keys(top))

solve(100)
```

```output
9183
```
