# Counting letters in the numbers one to a thousand

Solves [Project Euler 17](https://projecteuler.net/problem=17). See the
[spoiler warning](00-spoiler-warning.md).

`Str` has no length in toylang (see [Str](../../reference/types/str.md)), so this never spells
a number out at all: `ones_letters`, `teens_letters`, and `tens_letters` are lookup tables of
how many letters each piece *would* take, and `under_hundred` and `letters` combine them the
way English grammar combines the words -- an "and" only between a hundreds part and a nonzero
remainder. Summing `letters(1)` through `letters(1000)` by plain recursion would put 1000
frames on some backends' call stacks at once; chunking the sum into ten runs of a hundred
keeps every backend's stack shallow, the same concern that shaped [the largest palindrome
product](04-largest-palindrome-product.md)'s search.

```toylang
fn ones_letters(n: Int) -> Int = [0, 3, 3, 5, 4, 4, 3, 5, 5, 4][n]!

fn teens_letters(n: Int) -> Int = [3, 6, 6, 8, 8, 7, 7, 9, 8, 8][n - 10]!

fn tens_letters(n: Int) -> Int = [0, 0, 6, 6, 5, 5, 5, 7, 6, 6][n / 10]!

fn under_hundred(n: Int) -> Int =
    n
        | . == 0 -> 0 or
              . < 10 -> ones_letters(n) or
              . < 20 -> teens_letters(n) or
              tens_letters(n) + ones_letters(n % 10)

fn letters(n: Int) -> Int =
    n
        | . == 1000 -> 11 or
              . / 100 > 0 -> ones_letters(n / 100) + 7 + (n | . % 100 > 0 -> 3 or 0) + under_hundred(n % 100) or
              under_hundred(n)

fn inner_sum(p: {n: Int, last: Int}) -> Int =
    p | .n > .last -> 0 or letters(p.n) + inner_sum({n: p.n + 1, last: p.last})

fn outer_sum(g: Int) -> Int =
    g
        | . > 9 -> 0 or
              inner_sum({n: g * 100 + 1, last: g * 100 + 100}) + outer_sum(g + 1)

outer_sum(0)
```

```output
21124
```
