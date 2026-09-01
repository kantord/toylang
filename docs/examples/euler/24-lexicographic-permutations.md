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
fn factorial(n: Int) -> Int = n | . <= 1 -> 1 or . * factorial(. - 1)

fn remove_at(p: {v: Vec<Int>, i: Int}) -> Vec<Int> =
    p | .i == 0 -> tail(.v)! or [.v[0]!] + remove_at({v: tail(.v)!, i: .i - 1})

fn nth_perm(p: {remaining: Vec<Int>, idx: Int}) -> Vec<Int> =
    p
        | length(.remaining) == 0 -> [] or
              [.remaining[.idx / factorial(length(.remaining) - 1)]!] + nth_perm({remaining: remove_at({v: .remaining, i: .idx / factorial(length(.remaining) - 1)}), idx: .idx % factorial(length(.remaining) - 1)})

nth_perm({remaining: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], idx: 999999})
```

```output
[2,7,8,3,9,1,5,4,6,0]
```
