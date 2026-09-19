# Adding up the primes below a bound

Solves [Project Euler 10](https://projecteuler.net/problem=10). See the
[spoiler warning](00-spoiler-warning.md).

The sum, about 1.4e11, is past `Int`'s 2.1 billion ceiling, so each prime is widened with
`i64` and the total is an [Int64](../../reference/types/int64.md). Trial division over
`range(2000000)` is [problem 7](07-10001st-prime.md)'s shape -- 2, then only odd divisors --
and `select(is_prime(.))` keeps the primes. `range` is a stream, so the candidates are tested
one at a time and only the primes are ever held: `sum` reduces a `Vec`, so `collect` gathers
them just before it.

This page is a `slow` fragment. Two million trial divisions are a second or three on the
compiled and JIT backends, but half a minute on CPython, a minute on Lua, and minutes on jq,
against a suite that otherwise finishes in about ninety seconds. So the docs harness
(`tests/docs.rs`) type-checks and emits the fragment on every backend on every `just test`,
and only executes it under `just slow-test`, where all seven backends print the published
answer.

```toylang slow
fn has_divisor({n, d}: {n: Int, d: Int}) -> Bool =
    d * d <= n and
        (n % d == 0 or has_divisor({n: n, d: d + (d == 2 | . -> 1 or 2)}))

fn is_prime(n: Int) -> Bool = n >= 2 and not has_divisor({n: n, d: 2})

sum(collect(range(2000000) | select(is_prime(.)) | map(i64(.))))
```

```output
142913828922
```
