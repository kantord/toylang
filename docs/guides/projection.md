# Projecting over a dimension

Reading a field out of every entry of a dimension has one preferred spelling:

```toylang
[{name: "ada"}, {name: "bo"}][].name
```

```output
["ada","bo"]
```

`[].field` distributes the projection over a kept dimension: it keeps every entry and
reads the field out of each one. The same thing spelled through `map`, `map(.name)`, stays
legal -- the checker accepts both -- but it is demoted for this case. `map` is for
transforming each entry; reading a field out of an entry is a projection's job, so the
language teaches that job through the spelling that is only that job.



`map` earns its name when the body does more than read a field -- building a record,
computing an expression:

```toylang
[{name: "ada", age: 36}, {name: "bo", age: 9}]
    | map({name: .name, greeting: "hello " + .name})
```

```output
[{"name":"ada","greeting":"hello ada"},{"name":"bo","greeting":"hello bo"}]
```

The boundary is whether the body is a pure projection. Projection when it is; `map`
when the body does anything else.