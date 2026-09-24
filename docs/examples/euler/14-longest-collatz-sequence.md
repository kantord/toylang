# The longest Collatz chain under a million

Solves [Project Euler 14](https://projecteuler.net/problem=14). See the
[spoiler warning](00-spoiler-warning.md).

Chain terms pass 32 bits -- 432 starting values under a million go through a term wider than
`Int`, and simulating the old wraparound arithmetic even changes the winner -- so each term is
walked as an [Int64](../../reference/types/int64.md). `chain_len` walks one chain
tail-recursively, and the pipeline pairs every starting value with its chain length and hands
the pairs to [`max_by(.len)`](../../reference/builtins/max_by.md), which keeps the pair with
the longest chain. `max` alone is not the tool here: it reduces integers, and the answer
wanted is the starting value, not its chain length. `max_by` returns an `Opt`, unwrapped
with `!` because the range is not empty, and keeps the first of equal maxima, so a tie would go
to the smaller starting value. `range` counts from zero, so `select(. >= 1)` drops the
zero that is not a Collatz start.

This page is a `slow` fragment. The million chains are roughly 130 million recursive steps,
with no memoization possible -- there is no mutation, so nothing shares chain tails -- which
prices the interpreted backends out of the every-fragment suite. So the docs harness
(`tests/docs.rs`) type-checks and emits the fragment on every backend on every `just test`,
and only executes it under `just slow-test`, where all seven backends find the true winner --
837799, chain length 525 -- in a few seconds on the compiled backends.

```toylang slow
fn chain_len({ n, acc }: { n: Int64, acc: Int }) -> Int =
  n == 1
  | . -> acc or
    chain_len {
      n: n % 2 == 0 | . -> n / 2 or n * 3 + 1,
      acc: acc + 1
    };


collect(range 1000000)
| select(. >= 1)
| map { n: ., len: chain_len { n: i64(.), acc: 1 } }
| max_by(.len)!
```

```output
{"n":837799,"len":525}
```
