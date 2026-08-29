# fields

`fields(r)`, of type `{...} -> Vec<Str>`: the record's field names, in the order its type
declares them. Order is metadata rather than part of a record type's identity
([kantord/toylang#60](https://github.com/kantord/toylang/issues/60)), so two records of "the
same type" spelled with fields in a different order can disagree here -- each carries its own
checked order, and `fields` reads it off.

```toylang
fields({name: "ada", age: 36})
```

```output
["name","age"]
```

The order is the type's, not the literal's alphabetization or any other convention:

```toylang
fields({z: 1, a: 2, m: 3})
```

```output
["z","a","m"]
```

`fields` needs a record:

```toylang
fields(1)
```

```error
`fields` needs a record, found Int (at byte 7)
```
