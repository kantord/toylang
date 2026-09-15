# Counting letters in the numbers one to a thousand

Solves [Project Euler 17](https://projecteuler.net/problem=17). See the
[spoiler warning](00-spoiler-warning.md).

`Str` has no length in toylang (see [Str](../../reference/types/str.md)), so this never spells
a number out at all: `ones_letters`, `teens_letters`, and `tens_letters` are lookup tables of
how many letters each piece *would* take, and `under_hundred` and `letters` combine them the
way English grammar combines the words -- an "and" only between a hundreds part and a nonzero
remainder. `sum` reduces the full 1-to-1000 range directly in one call, a builtin reduction
rather than user recursion, so no chunking is needed.

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

sum(collect(range(1000)) | map(letters(1 + .)))
```

```output
21124
```
