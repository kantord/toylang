# Enums

A declared, closed set of named variants, nominal: the name is the identity, and consuming
one must handle every variant. As data an enum is plain JSON, never an opaque value
([ADR 0009](../../adr/0009-enums-are-json-native-single-key-wrappers.md)).

```toylang
enum Shape { Point, Circle{r: Int} }

{a: Shape.point, b: circle({r: 3})}
```

```output
{"a":"Point","b":{"Circle":{"r":3}}}
```

A unit variant carries nothing and is a bare string on the wire. A payload variant carries
one type and is the single-key wrapper. The payload can be any single type: a record
declared in braces (`Circle{r: Int}`), or anything else in parens the way a call passes a
non-record argument:

```case
enum_scalar_payload
```

A variant name starts with a capital letter; it is the identity used in a match pattern and
the JSON key for a payload variant. Construction is ordinary application of the lowercase
constructor the declaration derives -- `circle{r: 3}`, `celsius(21)` -- and the bare
constructor works while exactly one enum claims it; `Shape.point` is the qualified way out
when two do.

Consumption is the [match](../operators/match.md), which is closed-world: every variant
handled, or an `any()` arm for the rest. A program whose match misses a variant is refused:

```toylang
enum Shape { Point, Circle{r: Int} }

fn area_ish(s: Shape) -> Int = s | Circle{r} -> r * r

area_ish(Shape.point)
```

```error
a match over `Shape` must cover every variant or end in a default; missing `Point` (at byte 73)
```

Because the wire shape is plain JSON, an enum types input directly, and the input is
validated against the declared set: a string that names no variant, or a wrapper whose
payload misses the declared type, is refused before the program runs.

```case
enum_input
```

## An enum that contains itself

A payload may name the enum being declared, as long as it does so through a `Vec`. That is
what makes JSON's own shape expressible: an array case holds a `Vec` of the same type, which
is a heap indirection rather than a value with no end.

```case
enum_recursive_value
```

The rule is per occurrence, not per declaration, so `enum E { Safe(Vec<E>), Bad(E) }` accepts
`safe` and still refuses `bad`: a bare self-reference is a layout that contains itself.

```toylang
enum Json { Arr(Vec<Json>), Num(Int), Node{next: Json} }

Json.num(1)
```

```error
type `Json` is written in terms of itself (at byte 49)
```
