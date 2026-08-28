# select

`select(pred)`: keeps the entries of its subject for which `pred` is true, and drops the
rest. A narrowing selection in the glossary's terms: the dimension survives at reduced
extent. The subject arrives through a pipe, and inside the predicate `.` is the entry being
judged.

```toylang
[1, 2, 3] | select(. >= 2)
```

```output
[2,3]
```

When the entries are records, a projection like `.age` reaches into the entry under
judgement, because `.` is that entry:

```case
adults
```

`select` works over a `Stream` subject too, and stays in the effect layer: a stream in, a
stream out, one entry judged at a time. What the predicate cannot do is read a source
(`inputs`, `lines`) itself, since it runs once per entry and stdin cannot be read once per
entry.

`select` is not special syntax. It is an ordinary name applied to one argument, reachable
because application binds the way it does; the same goes for [`map`](map.md).
