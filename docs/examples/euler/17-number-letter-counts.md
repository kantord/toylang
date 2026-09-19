# Counting letters in the numbers one to a thousand

Solves [Project Euler 17](https://projecteuler.net/problem=17). See the
[spoiler warning](00-spoiler-warning.md).

Each number is spelled out the way the problem counts it, letters only, no spaces or
hyphens: `ones`, `teens`, and `tens` are the word tables, and `under_hundred` and `words`
combine them the way English does, with an "and" only between a hundreds part and a nonzero
remainder. A `Str` has no length of its own, but
[`chars`](../../reference/builtins/chars.md) turns one into a `Vec<Char>` that does, so
`length(chars(words(n)))` is a number's letter count.

```toylang
fn ones(n: Int) -> Str =
  [
    "",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine"
  ][n]!


fn teens(n: Int) -> Str =
  [
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen"
  ][n - 10]!


fn tens(n: Int) -> Str =
  [
    "",
    "",
    "twenty",
    "thirty",
    "forty",
    "fifty",
    "sixty",
    "seventy",
    "eighty",
    "ninety"
  ][n / 10]!


fn under_hundred(n: Int) -> Str =
  n | . < 10 -> ones n or . < 20 -> teens n or tens n + ones(n % 10)


fn words(n: Int) -> Str =
  n
  | . == 1000 -> "onethousand" or
    . < 100 -> under_hundred n or
    . % 100 == 0 -> ones(n / 100) + "hundred" or
    ones(n / 100) + "hundredand" + under_hundred(n % 100)


sum(collect(range 1000) | map length(chars words(. + 1)))
```

```output
21124
```
