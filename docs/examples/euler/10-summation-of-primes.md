# Adding up the primes below a bound

Solves [Project Euler 10](https://projecteuler.net/problem=10). See the
[spoiler warning](00-spoiler-warning.md).

The sum, about 1.4e11, is past `Int`'s 2.1 billion ceiling, so the running total is carried in
[Int64](../../reference/types/int64.md). The width blocker
([kantord/toylang#38](https://github.com/kantord/toylang/issues/38)) is what kept this page out
until [kantord/toylang#83](https://github.com/kantord/toylang/issues/83) landed. Trial division
over `range(2000000)` is [problem 7](07-10001st-prime.md)'s shape -- 2, then only odd divisors;
`select(is_prime(.) == 1)` keeps the primes, and `sum_range` adds them by halving the range,
each half summed as an `Int64` so the total never wraps. `range` is a stream source since
[kantord/toylang#137](https://github.com/kantord/toylang/issues/137), and `sum_range`'s
halving needs random access into a held `Vec`, so the pipeline reifies it up front with
`collect` (the eager spelling).

This page is a `slow` fragment. Two million trial divisions are a second or three on the
compiled and JIT backends, but half a minute on CPython, a minute on Lua, and minutes on jq
([kantord/toylang#90](https://github.com/kantord/toylang/issues/90)), against a suite that
otherwise finishes in about ninety seconds. So
[the docs harness](../../reference/syntax/programs.md) type-checks and emits the fragment on
every backend on every `just test`, and only executes it under `just slow-test`, where all
seven backends print the published answer.

```toylang slow
fn has_divisor(p: {n: Int, d: Int}) -> Int =
    0 if p.d * p.d > p.n else
        1 if p.n % p.d == 0 else
        has_divisor({n: p.n, d: p.d + (1 if p.d == 2 else 2)})

fn is_prime(n: Int) -> Int =
    0 if n < 2 else 0 if has_divisor({n: n, d: 2}) == 1 else 1

fn sum_range(p: {v: Vec<Int>, lo: Int, hi: Int}) -> Int64 =
    i64(p.v[p.lo]!) if p.hi - p.lo == 1 else
        sum_range({v: p.v, lo: p.lo, hi: (p.lo + p.hi) / 2}) +
            sum_range({v: p.v, lo: (p.lo + p.hi) / 2, hi: p.hi})

collect(range(2000000)) | select(is_prime(.) == 1) | sum_range({v: ., lo: 0, hi: length(.)})
```

```output
142913828922
```
