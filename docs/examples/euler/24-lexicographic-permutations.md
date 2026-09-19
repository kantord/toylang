# The millionth lexicographic permutation of 0123456789

Solves [Project Euler 24](https://projecteuler.net/problem=24). See the
[spoiler warning](00-spoiler-warning.md).

No search: the factorial number system picks each digit directly. With `k` digits still
unplaced, the next `(k-1)!` permutations share the first remaining digit, so dividing the
target index by `(k-1)!` gives that digit's position `i` in what's left, and the remainder
carries into the next digit; the slices `remaining[:i] + remaining[i + 1:]` are the list with
that digit dropped. Ten digits keep the recursion to depth ten. Joined, the digits of the
millionth permutation (index 999999, since the first is index zero) make 2783915460, past
`Int` though inside `Int64`, so the result is left as the `Vec<Int>` of digits, the shape
[problem 13](13-large-sum.md) also prints a number too wide for `Int` in.

```toylang
fn factorial(n: Int) -> Int = n | . <= 1 -> 1 or . * factorial(. - 1)

fn nth_perm({remaining, idx}: {remaining: Vec<Int>, idx: Int}) -> Vec<Int> =
    let block = factorial(length(remaining) - 1)
    let i = idx / block
    remaining
        | length(remaining) == 0 -> [] or
              [remaining[i]!] +
                  nth_perm(
                      {
                          remaining: remaining[:i] + remaining[i + 1:],
                          idx: idx % block
                      }
                  )

nth_perm({remaining: [0, 1, 2, 3, 4, 5, 6, 7, 8, 9], idx: 999999})
```

```output
[2,7,8,3,9,1,5,4,6,0]
```
