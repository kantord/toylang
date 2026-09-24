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
[1, 2] | map { n: ., squared: . * . }
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

A bare name bound to a closure value (one built with [`$`](../operators/placeholder.md) and
handed in as a parameter) is applied to each entry instead of rebinding `.`. Only a bare name
that is already a function is read this way; anything else keeps the `.` meaning above. The
closure's result type is the element type of what `map` yields, so it may differ from the
entries':

```case
closure_map
```

A closure built in a tail-recursive function captures the values its locals had when it was
built, not the variables, which `closure_map_carried_through_tail_call` pins.
