# Summing the corners of a number spiral

Solves [Project Euler 28](https://projecteuler.net/problem=28). See the
[spoiler warning](00-spoiler-warning.md).

No spiral gets built. Ring `i` out from the center 1 has side length `2i + 1`, and its four
corners are `(2i+1)^2` and that same square minus `2i`, `4i`, and `6i` -- one step around each
side of the ring. Summed, the `12i` and one `2i`-multiple's worth of cross terms collapse to
`4*(2i+1)^2 - 12i`, so each ring's contribution is a closed-form expression rather than four
separate lookups, and `sum` adds the 500 rings to the centre's `1`.

```toylang
fn ring_sum(i: Int) -> Int = 4 * (2 * i + 1) * (2 * i + 1) - 12 * i

1 + sum(collect(range 501) | select(. >= 1) | map ring_sum(.))
```

```output
669171001
```
