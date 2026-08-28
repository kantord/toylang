# unlines

`unlines(v)`, of type `Vec<Str> -> Str`: joins the entries with `"\n"`, with no trailing
newline of its own. The way a list of lines becomes printable text, since a top-level `Str`
prints raw.

```toylang
unlines(["ada", "bo"])
```

```output
ada
bo
```

`unlines` is not a builtin: it is toylang source, defined in `prelude.toy` as an ordinary
recursive function, and it compiles the way any user function does. The name is Haskell's.
