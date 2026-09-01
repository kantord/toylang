# Summing the corners of a number spiral

Solves [Project Euler 28](https://projecteuler.net/problem=28). See the
[spoiler warning](00-spoiler-warning.md).

No spiral gets built. Ring `i` out from the center 1 has side length `2i + 1`, and its four
corners are `(2i+1)^2` and that same square minus `2i`, `4i`, and `6i` -- one step around each
side of the ring. Summed, the `12i` and one `2i`-multiple's worth of cross terms collapse to
`4*(2i+1)^2 - 12i`, so each ring's contribution is a closed-form expression rather than four
separate lookups. `sum_rings` halves its way down from ring 1 to ring 500 the way
[problem 21](21-amicable-numbers.md)'s does, rather than adding rings one at a time.

```toylang
fn ring_sum(i: Int) -> Int = 4 * (2 * i + 1) * (2 * i + 1) - 12 * i

fn sum_rings(p: {lo: Int, hi: Int}) -> Int =
    p
        | .hi - .lo == 1 -> ring_sum(.lo) or
              sum_rings({lo: .lo, hi: (.lo + .hi) / 2}) + sum_rings({lo: (.lo + .hi) / 2, hi: .hi})

1 + sum_rings({lo: 1, hi: 501})
```

```output
669171001
```
