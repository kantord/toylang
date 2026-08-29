# Summing a hundred large numbers (skipped)

Skipped. [Project Euler 13](https://projecteuler.net/problem=13) gives a hundred fifty-digit
numbers with no source but Project Euler itself -- the same open question as problem 8, this
time for a hundred numbers rather than one string. Storing each as a `Vec<Int>` of digits
would sidestep toylang's 32-bit `Int` ceiling, but not the data-blob question. See
[kantord/toylang#39](https://github.com/kantord/toylang/issues/39) and the
[spoiler warning](00-spoiler-warning.md).
