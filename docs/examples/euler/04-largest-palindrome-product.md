# Largest palindrome from two three-digit factors

Solves [Project Euler 4](https://projecteuler.net/problem=4). See the
[spoiler warning](00-spoiler-warning.md).

There is no `max` builtin, so `max_vec` folds a `Vec<Int>` by hand, one entry peeled off with
`tail` per call, which means its recursion depth tracks the `Vec`'s length. Run flat over all
~400,000 candidate products, that depth would risk a backend's call-stack limit, so the search
is nested instead: an inner `max` per fixed first factor (at most 900 candidates), then an
outer `max` over each row's winner (900 rows), each `max_vec` sentinel-padded with `0` so an
empty row never runs out of entries.

```toylang
fn reverse_num(p: {n: Int, acc: Int}) -> Int =
    p
        | .n == 0 -> p.acc or
              reverse_num({n: p.n / 10, acc: p.acc * 10 + p.n % 10})

fn max2(p: {a: Int, b: Int}) -> Int = p | .a > .b -> p.a or p.b

fn max_vec(xs: Vec<Int>) -> Int =
    xs | length(.) == 1 -> xs[0]! or max2({a: xs[0]!, b: max_vec(tail(xs)!)})

fn row_max(a: Int) -> Int =
    max_vec(
        [0] +
            (
                collect(range(1000))
                    | select(. >= a)
                    | map(a * .)
                    | select(. == reverse_num({n: ., acc: 0}))
            )
    )

max_vec(collect(range(1000)) | select(. >= 100) | map(row_max(.)))
```

```output
906609
```
