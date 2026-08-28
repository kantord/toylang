# Values

A toylang program is one expression, and its value is what prints. The shortest program is
a value by itself:

```toylang
"hello world"
```

```output
hello world
```

No `main`, no print call. Every fragment on these pages is a complete program the test
suite runs on all seven compiler backends -- paste any of them into a file and
`toylang run FILE` does the rest.

## Numbers

`Int` is a 32-bit integer. The operators are `+ - * / %`, with the precedence you expect:

```toylang
2 + 3 * 4
```

```output
14
```

Division truncates toward zero, overflow wraps around at 32 bits, and dividing by zero is
refused at runtime. The details live in the reference, under [Int](../reference/types/int.md)
and [arithmetic](../reference/operators/arithmetic.md).

## Strings

Double quotes, C-style escapes, and `+` to concatenate:

```toylang
"hello " + "world"
```

```output
hello world
```

A number does not concatenate as-is; `str(n)` renders it first:

```toylang
"the answer is " + str(6 * 7)
```

```output
the answer is 42
```

## Vecs

`[1, 2, 3]` builds a `Vec<Int>`: one dimension of entries, all the same type. It prints as
JSON, and everything nested inside a value prints as JSON -- only a top-level `Str` prints
raw, as above.

```toylang
[[1, 2], [3]]
```

```output
[[1,2],[3]]
```

## Choosing

Comparisons (`==`, `!=`, `<`, `<=`, `>`, `>=`) produce a `Bool`, and the conditional is an
expression, spelled Python's way:

```toylang
"big" if 10 > 5 else "small"
```

```output
big
```

Chains read left to right, so a cascade of cases lays out flat; you will meet one in
[chapter 3](03-pipes.md).

Next: [records](02-records.md), the type where values get names.
