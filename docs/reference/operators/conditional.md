# The conditional

Retired in [kantord/toylang#155](https://github.com/kantord/toylang/issues/155). There was a
ternary, `A if C else B`, spelled Python's way; it is gone. One conditional form remains, the
guard-arm [match](match.md), and the old ternary is the match's two-arm shape:

```
"big" if 10 > 5 else "small"    10 > 5 | . -> "big" or "small"
```

A cascade of `if`/`else` branches is the same cascade of guard arms, each condition next to
its own result and a bare default at the end. [Chapter 1](../../tutorial/01-values.md)
introduces the spelling, [matching](../../guides/matching.md) the shape.
