# Pipes, select, and map

`x | f` feeds the left side to the right, and inside the right side `.` is what arrived.
The two workhorses that follow a pipe are `select`, which keeps the entries a predicate
approves, and `map`, which transforms each entry:

```toylang
[1, 2, 3, 4] | select(. % 2 == 0) | map(. * 10)
```

```output
[20,40]
```

When the entries are records, projections reach into the current entry, because `.` is that
entry:

```toylang
fn adults(users: Vec<{name: Str, age: Int}>) -> Vec<Str> =
    users | select(.age >= 18) | .[].name

adults([{name: "ada", age: 36}, {name: "bo", age: 9}])
```

```output
["ada"]
```

`[].name` is the projection spelling: it reads a field through the dimension directly.
The same projection inside a `map` body, `map(.name)`, is legal but demoted -- `map`
is for transforming entries, not for reading a field out of them. They are verified
identical.

## Reaching in by position

An index collapses a dimension to one entry -- `[0]` from the front, `[-1]` from the back.
What comes back may be absent (the index can be out of range), so it is an `Opt`, and `!`
insists the entry is there:

```toylang
["ada", "bo", "cy"][-1]!
```

```output
cy
```

Without the `!`, absence flows to the output as `null`. With it, an absent value is refused
at runtime. Chapter 1's `Vec` pages in the reference cover the whole spec story:
[index specs](../reference/operators/specs.md), [Opt](../reference/types/opt.md).

## Putting it together

`collect(range(n))` makes `[0 .. n-1]`, and `join_lines` joins a `Vec<Str>` into printable
lines. (`range` itself is a stream, counted one entry at a time; `collect` is the eager
spelling that turns it into the `Vec` here.) FizzBuzz is one pipeline:

```case
fizzbuzz
```

The same cascade reappears chained with `or` instead of nested `else` in
[matching](06-matching.md), once enums have introduced the other kind of arm.

Next: [enums](04-enums.md), for data that is one of a known set of shapes.
