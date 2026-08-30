# join

`join(v)`, of type `Vec<Str> -> Str`: concatenates the entries with no separator between
them, unlike [`join_lines`](join_lines.md), which inserts `"\n"`.

```toylang
join(["ada", "bo"])
```

```output
adabo
```

An empty `Vec` joins to the empty `Str`:

```toylang
join([])
```

```output

```

`join` is not a builtin: it is toylang source, defined in `prelude.toy` as an ordinary
recursive function, and it compiles the way any user function does.
