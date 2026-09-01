# The longest Collatz chain under a million

Solves [Project Euler 14](https://projecteuler.net/problem=14). See the
[spoiler warning](00-spoiler-warning.md).

Chain terms pass 32 bits -- 432 starting values under a million go through a term wider than
`Int`, and simulating the old wraparound arithmetic even changes the winner -- so each term is
walked as an [Int64](../../reference/types/int64.md), which is the half of
[kantord/toylang#38](https://github.com/kantord/toylang/issues/38) that
[kantord/toylang#83](https://github.com/kantord/toylang/issues/83) closed. `chain_len` walks
one chain tail-recursively, `better` keeps the longer chain, and `longest` compares every
starting value by halving the range, so each candidate's chain is walked once.

This page is a `slow` fragment. The million chains are roughly 130 million recursive steps,
with no memoization possible -- there is no mutation, so nothing shares chain tails -- which
prices the interpreted backends out of the every-fragment suite
([kantord/toylang#90](https://github.com/kantord/toylang/issues/90)). So
[the docs harness](../../reference/syntax/programs.md) type-checks and emits the fragment on
every backend on every `just test`, and only executes it under `just slow-test`, where all
seven backends find the true winner -- 837799, chain length 525 -- in a few seconds on the
compiled backends.

```toylang slow
fn chain_len(p: {n: Int64, acc: Int}) -> Int =
    p
        | .n == i64(1) -> p.acc or
        chain_len({n: (p | .n % 2 == i64(0) -> p.n / 2 or p.n * 3 + 1), acc: p.acc + 1})

fn better(p: {a: {n: Int, len: Int}, b: {n: Int, len: Int}}) -> {n: Int, len: Int} =
    p | .a.len >= .b.len -> p.a or p.b

fn longest(p: {lo: Int, hi: Int}) -> {n: Int, len: Int} =
    p
        | .hi - .lo == 1 -> {n: p.lo, len: chain_len({n: i64(p.lo), acc: 1})} or
        better(
            {
                a: longest({lo: p.lo, hi: (p.lo + p.hi) / 2}),
                b: longest({lo: (p.lo + p.hi) / 2, hi: p.hi})
            }
        )

longest({lo: 1, hi: 1000000})
```

```output
{"n":837799,"len":525}
```
