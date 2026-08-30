# When Fibonacci reaches a thousand digits (skipped)

Skipped. [Project Euler 25](https://projecteuler.net/problem=25) asks for the index of the
first Fibonacci term with 1000 digits, and that term is nowhere near `Int64`'s reach either --
a 1000-digit number is on the order of 10^999, past even a 64-bit integer's roughly 1.8e19
ceiling by nearly a thousand orders of magnitude. Unlike [problem 29](29-distinct-powers.md),
there is no factoring trick that sidesteps holding the number itself: consecutive Fibonacci
terms share no multiplicative structure to fold away, only an additive recurrence, so the
digits have to exist somewhere for the length check to mean anything. This is the BigInt half
of [kantord/toylang#38](https://github.com/kantord/toylang/issues/38)'s two-step plan, not
yet built. See the [spoiler warning](00-spoiler-warning.md).
