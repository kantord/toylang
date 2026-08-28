# map

`map(expr)`: applies `expr` to every entry of its subject, yielding a `Vec` of the results
(or a `Stream`, when the subject is one). The subject arrives through a pipe, and inside the
body `.` is the current entry.

```toylang
[1, 2, 3] | map(. * 2)
```

```output
[2,4,6]
```

The body is any expression, so mapping into a record literal builds one per entry. A record
literal argument may drop its parentheses, the way any application of a record may:

```toylang
[1, 2] | map {n: ., squared: . * .}
```

```output
[{"n":1,"squared":1},{"n":2,"squared":4}]
```

Over records, projections reach into the current entry:

```case
map_records
```

Like [`select`](select.md), `map` accepts a `Stream` subject and yields a `Stream` back,
one entry at a time; and like `select`'s predicate, its body cannot read a source, since the
body runs once per entry.
