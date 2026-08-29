# Enums

A declared, closed set of named variants, nominal: the name is the identity, and consuming
one must handle every variant. As data an enum is plain JSON, never an opaque value
([ADR 0009](../../adr/0009-enums-are-json-native-single-key-wrappers.md)).

```toylang
enum Shape { point, circle{r: Int} }

{a: Shape.point, b: circle({r: 3})}
```

```output
{"a":"point","b":{"circle":{"r":3}}}
```

A unit variant carries nothing and is a bare string on the wire. A payload variant carries
one type and is the single-key wrapper. The payload can be any single type: a record
declared in braces (`circle{r: Int}`), or anything else in parens the way a call passes a
non-record argument:

```case
enum_scalar_payload
```

A variant name is data, so it is lowercase, like a field. Construction is ordinary
application of the constructor the declaration derives -- `circle{r: 3}`, `celsius(21)` --
and the bare unit-variant name works while exactly one enum claims it; `Shape.point` is the
qualified way out when two do.

Consumption is the [match](../operators/match.md), which is closed-world: every variant
handled, or an `any()` arm for the rest. A program whose match misses a variant is refused:

```toylang
enum Shape { point, circle{r: Int} }

fn area_ish(s: Shape) -> Int = s | circle{r} -> r * r

area_ish(Shape.point)
```

```error
a match over `Shape` must cover every variant or end in a default; missing `point` (at byte 73)
```

Because the wire shape is plain JSON, an enum types input directly, and the input is
validated against the declared set: a string that names no variant, or a wrapper whose
payload misses the declared type, is refused before the program runs.

```case
enum_input
```
