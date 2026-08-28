# The pipe

`x | f` feeds the left side to the right side, and inside the right side `.` is the value
that arrived. Stages chain left to right, so a transformation reads in the order it
happens:

```toylang
[1, 2, 3] | select(. >= 2) | map(. * 10)
```

```output
[20,30]
```

Three forms take their subject from the pipe rather than from an argument:
[`select`](../builtins/select.md), [`map`](../builtins/map.md), and the
[match](match.md)'s arm chain. Everything else -- `extent`, `str`, a user function -- takes
its data as an ordinary argument, and inside a pipe stage is applied to whatever expression
mentions `.`:

```toylang
[10, 20, 30][1]! | str(.)
```

```output
20
```

`.` rebinds at each boundary that introduces a subject: a pipe stage, a `map` or `select`
body (where it is the current entry), a match arm over a bare payload (where it is the
payload). A projection like `.age` inside those positions reaches into whatever `.`
currently is.

Piping is plumbing, not effect: on a `Vec` everything is ordinary values, and on a
[`Stream`](../types/stream.md) the same spelling stays one fused chain from source to sink.
