# The biggest prime dividing a number

Solves [Project Euler 3](https://projecteuler.net/problem=3). See the
[spoiler warning](00-spoiler-warning.md).

Skipped until [Int64 landed](../../reference/types/int64.md)
([kantord/toylang#83](https://github.com/kantord/toylang/issues/83)): the number the problem
names does not fit toylang's 32-bit `Int`, and the checker used to refuse the literal
outright. It still would, anywhere else -- the literal below is legal only because the
record it sits in is checked against `{n: Int64, d: Int64}`, the position-resolved rule the
type shipped with.

Trial division, the same recursion-instead-of-loop shape
[problem 7](07-10001st-prime.md) uses: divide out each divisor while it divides, step to the
next odd candidate otherwise, and stop when the divisor's square passes what is left --
whatever remains then is the largest prime factor. The divisor steps 2, 3, 5, 7, ... so the
recursion is about 740 calls deep at its deepest, inside every backend's stack.

```toylang
fn largest(p: {n: Int64, d: Int64}) -> Int64 =
    p
        | .d * .d > .n -> p.n or
              .n % .d == 0 -> largest({n: p.n / p.d, d: p.d}) or
              largest({n: p.n, d: p.d + (p | .d == 2 -> 1 or 2)})

largest({n: 600851475143, d: 2})
```

```output
6857
```

Every value the program touches -- the number itself, each quotient, each divisor's square --
stays inside 2^53, so the jq backend's
[documented Int64 precision envelope](../../reference/types/int64.md) holds and all seven
backends agree exactly.
