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

Over a dimension of records there are two spellings, verified identical: distribute the
projection over a kept dimension, or map it.

```toylang
[{n: 1}, {n: 2}][].n
```

```output
[1,2]
```

```toylang
[{n: 1}, {n: 2}] | map(.n)
```

```output
[1,2]
```

Both are current style; jq has the same pair (`.[].name` versus `map(.name)`). Field names
come from data, so a field spelled with a capital letter projects like any other.
