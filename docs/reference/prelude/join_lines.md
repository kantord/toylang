# join_lines

`join_lines(v)`, of type `Vec<Str> -> Str`: joins the entries with `"\n"`, with no trailing
newline of its own. The way a list of lines becomes printable text, since a top-level `Str`
prints raw.

```toylang
join_lines(["ada", "bo"])
```

```output
ada
bo
```

`join_lines` is not a builtin: it is toylang source, defined in `prelude.toy` as an ordinary
recursive function, and it compiles the way any user function does.