# How many powers are truly distinct

Solves [Project Euler 29](https://projecteuler.net/problem=29). See the
[spoiler warning](00-spoiler-warning.md).

`a^b` for `a` up to 100 and `b` up to 100 overflows past any fixed-width integer almost
immediately -- `2^100` alone is nowhere near `Int64`'s reach, let alone `Int`'s -- so nothing
here ever computes a power. Two numbers `a^b` and `c^d` coincide only when `a` and `c` are
powers of a common base, so every `a` is rewritten as `root^mult` with `mult` as large as
possible. `powers` tables every `root^mult` with `root` from 2 to 10 and `mult` from 6 down to
2 (a root of 11 or more squared already passes 100), then every `a` as its own first power,
so the first table entry whose value is `a` is the rewrite, and
[`first`](../../reference/builtins/first.md) always finds one. `a^b` then stands for the pair
`(root, mult * b)`, and counting distinct pairs counts distinct powers without ever forming
one. `root <= 100` and `mult * b <= 600` pack losslessly into one `Int` key
(`root * 1000 + mult * b`), which is where this stops being a `BigInt` problem
([contrast problem 25](25-1000-digit-fibonacci-number.md), which has no such trick available).

That still leaves counting *distinct* keys among the 9801 `(a, b)` pairs, and
[`sort`](../../reference/builtins/sort.md) makes that a direct job: pack all 9801 keys, sort
them, and count the adjacent equal pairs. A duplicate in a sorted list always sits next to
its twin, so subtracting that count from the total length gives the distinct count.
`keys_for_base` expands one base's 99 exponents, `all_keys` flattens those rows into the full
9801-key set, and `adjacent_dup_count` is the number of positions whose right-hand neighbour
is equal, so no key is ever compared against more than its neighbour in the sorted order.

```toylang
type Power = { value: Int, root: Int, mult: Int }

fn ipow({ r, m }: { r: Int, m: Int }) -> Int =
  m | . == 0 -> 1 or r * ipow { r: r, m: m - 1 }

fn powers_with_mult(
  { m, roots }: { m: Int, roots: Int }
) -> Vec<Power> =
  collect range(roots - 1)
  | map(. + 2)
  | map { value: ipow { r: ., m: m }, root: ., mult: m }

fn powers() -> Vec<Power> =
  flatten(
    collect(range 5)
    | map(6 - .)
    | map(powers_with_mult { m: ., roots: 10 })
  ) +
    powers_with_mult { m: 1, roots: 100 }

fn keys_for_base(
  { a, bs, table }: { a: Int, bs: Vec<Int>, table: Vec<Power> }
) -> Vec<Int> =
  let p = first(table | select(.value == a))!
  bs | map(p.root * 1000 + p.mult * .)

fn all_keys(top: Int) -> Vec<Int> =
  let table = powers()
  let bases = collect range(top - 1) | map(. + 2)
  flatten(
    bases | map(keys_for_base { a: ., bs: bases, table: table })
  )

fn adjacent_dup_count(sorted: Vec<Int>) -> Int =
  length(
    collect range(length sorted - 1)
    | select(sorted[. + 1]! == sorted[.]!)
  )

fn distinct_count(v: Vec<Int>) -> Int =
  length v - adjacent_dup_count(sort v)

distinct_count(all_keys 100)
```

```output
9183
```
