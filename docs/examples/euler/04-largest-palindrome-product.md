# Largest palindrome from two three-digit factors

Solves [Project Euler 4](https://projecteuler.net/problem=4). See the
[spoiler warning](00-spoiler-warning.md).

The search is one flat pass: `row_candidates` collects the palindromes a fixed first factor
forms, every row's candidate list is flattened into one list, and a single `max` call reduces
it. No sentinel is needed -- an empty row's candidate list just contributes nothing to the
flattened whole, and `max`'s `Opt` result is unwrapped with `!` because the flattened list is
provably non-empty.

```toylang
fn reverse_num({ n, acc }: { n: Int, acc: Int }) -> Int =
  n == 0
  | . -> acc or reverse_num { n: n / 10, acc: acc * 10 + n % 10 }


fn row_candidates(a: Int) -> Vec<Int> =
  collect(range 1000)
  | select(. >= a)
  | map(a * .)
  | select(. == reverse_num { n: ., acc: 0 })


max(
  flatten(
    collect(range 1000) | select(. >= 100) | map row_candidates(.)
  )
)!
```

```output
906609
```
