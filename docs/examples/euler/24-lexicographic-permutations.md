# The millionth lexicographic permutation of 0123456789

Solves [Project Euler 24](https://projecteuler.net/problem=24). See the
[spoiler warning](00-spoiler-warning.md).

No search: the factorial number system picks each digit directly. With `k` digits still
unplaced, the next `(k-1)!` permutations share the first remaining digit, so dividing the
target index by `(k-1)!` gives that digit's position in what's left, and the remainder carries
into the next digit. `remove_at` drops a digit out of the remaining list functionally, by
rebuilding everything after it. Ten digits keep every recursion here to depth ten at most.
The millionth permutation (index 999999, since the first is index zero) turns out to start
past `Int`'s comfortable range as a single number, so the result is left as the `Vec<Int>` of
digits, the same call [problem 13](13-large-sum.md) makes for a number too wide to print
whole.

```toylang
fn factorial(n: Int) -> Int = 1 if n <= 1 else n * factorial(n - 1)

fn remove_at(p: {v: Vec<Int>, i: Int}) -> Vec<Int> =
    tail(p.v)! if p.i == 0 else
        [p.v[0]!] + remove_at({v: tail(p.v)!, i: p.i - 1})

fn nth_perm(p: {remaining: Vec<Int>, idx: Int}) -> Vec<Int> =
    [] if extent(p.remaining) == 0 else
        [p.remaining[p.idx / factorial(extent(p.remaining) - 1)]!] +
            nth_perm(
                {
                    remaining: remove_at(
                        {
                            v: p.remaining,
                            i: p.idx / factorial(extent(p.remaining) - 1)
                        }
                    ),
                    idx: p.idx % factorial(extent(p.remaining) - 1)
                }
            )

nth_perm({remaining: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], idx: 999999})
```

```output
[2,7,8,3,9,1,5,4,6,0]
```
