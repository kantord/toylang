# The longest Collatz chain under a million

Solves [Project Euler 14](https://projecteuler.net/problem=14). See the
[spoiler warning](00-spoiler-warning.md).

Chain terms pass 32 bits -- 432 starting values under a million go through a term wider than
`Int`, and simulating the old wraparound arithmetic even changes the winner -- so each term is
walked as an [Int64](../../reference/types/int64.md). `chain_len` walks one chain
tail-recursively, `better` keeps the longer chain, and `longest` compares every starting value
by halving the range, so each candidate's chain is walked once. `max` is not the tool here:
it reduces integers, and the answer wanted is the starting value, not its chain length.

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
    }


fn better(
  { a, b }: { a: { n: Int, len: Int }, b: { n: Int, len: Int } }
) -> { n: Int, len: Int } =
  a.len >= b.len | . -> a or b


fn longest(
  { lo, hi }: { lo: Int, hi: Int }
) -> { n: Int, len: Int } =
  let mid = (lo + hi) / 2

  hi - lo == 1
  | . -> { n: lo, len: chain_len { n: i64 lo, acc: 1 } } or
    better {
      a: longest { lo: lo, hi: mid },
      b: longest { lo: mid, hi: hi }
    }


longest { lo: 1, hi: 1000000 }
```

```output
{"n":837799,"len":525}
```
