# Counting lattice paths across a grid

Solves [Project Euler 15](https://projecteuler.net/problem=15). See the
[spoiler warning](00-spoiler-warning.md).

A right-and-down path across a 20x20 grid is 40 steps of which exactly 20 go right, so the
count is `C(40, 20)`. `choose_2n_n` builds it as the running product
`C(n + i, i) = C(n + i - 1, i - 1) * (n + i) / i`, exact at every step because each partial
product is itself a binomial coefficient. The answer, 137,846,528,820, is what kept this page
skipped before [Int64](../../reference/types/int64.md) existed: it passes `Int`'s 2.1e9
ceiling by two orders of magnitude. The largest intermediate, `C(39, 19) * 40`, is about
2.8e12, inside jq's 2^53 envelope, so all seven backends agree.

```toylang
fn choose_2n_n(
  { n, i, acc }: { n: Int, i: Int, acc: Int64 }
) -> Int64 =
  i
  | . > n -> acc or
    choose_2n_n { n: n, i: i + 1, acc: acc * i64(n + i) / i64 i }

choose_2n_n { n: 20, i: 1, acc: 1 }
```

```output
137846528820
```
