# Projection

`.name` reads a field out of a record -- choosing which part, where
[selection](specs.md) chooses which entries.

```toylang
{name: "ada", age: 36}.age
```

```output
36
```

Projections chain through nested records, and apply to whatever `.` currently is inside a
pipe stage, a `map` or `select` body, or a bare-payload match arm:

```case
nested_field_access
```

Over a dimension of records, `[].field` is the projection spelling: it distributes the
projection over a kept dimension:

```toylang
[{n: 1}, {n: 2}][].n
```

```output
[1,2]
```

`map(.n)` is the same transformation and stays legal, but for a pure projection it is
demoted: `map` is for transforming each entry, not for reading a field out of it. jq has
the same pair (`.[].name` versus `map(.name)`). Field names come from data, so a field
spelled with a capital letter projects like any other.
