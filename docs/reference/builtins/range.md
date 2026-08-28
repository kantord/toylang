# range

`range(n)`, of type `Int -> Vec<Int>`: the integers from zero up to but not including `n`.
Zero-based, matching jq, Python, and the language's own indices.

```toylang
range(4)
```

```output
[0,1,2,3]
```

A zero or negative argument yields the empty `Vec`, not an error:

```toylang
extent(range(-3))
```

```output
0
```

The result is an ordinary `Vec<Int>`, so everything that works on a `Vec` works on it:

```toylang
range(5) | select(. >= 2) | map(. * 10)
```

```output
[20,30,40]
```
