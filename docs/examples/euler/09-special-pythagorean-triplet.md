# A Pythagorean triplet that sums to 1000

Solves [Project Euler 9](https://projecteuler.net/problem=9). See the
[spoiler warning](00-spoiler-warning.md).

`row` searches every `b` for one fixed `a`; the outer expression maps `row` over every `a`
and flattens with `flatten`, then takes the first (and, for this input, only) hit. No `max` or
fold needed here, since the triplet turns out to be unique.

```toylang
fn abc(p: {a: Int, b: Int}) -> Int = p.a * p.b * (1000 - p.a - p.b)

fn row(a: Int) -> Vec<Int> =
    collect(range(1000))
        | select(. > a and . < 1000 - a)
        | select(a * a + . * . == (1000 - a - .) * (1000 - a - .))
        | map(abc({a: a, b: .}))

flatten(collect(range(1000)) | select(. >= 1) | map(row(.)))[0]!
```

```output
31875000
```
