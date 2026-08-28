# Enums

Some data is one of a known set of shapes: a status is `active` or `inactive`, a shape is a
point or a circle with a radius. An enum declares the set, closed:

```toylang
enum Status { active, inactive }

Status.active
```

```output
"active"
```

Note what printed: a bare string. An enum is plain JSON on the wire -- a unit variant is a
string, a payload variant is a single-key wrapper -- so enums type real wire data directly
rather than inventing a private encoding.

```toylang
enum Shape { point, circle{r: Int} }

circle{r: 3}
```

```output
{"circle":{"r":3}}
```

Constructing a variant is ordinary application of the constructor the declaration derives.
The bare name (`circle{r: 3}`, `active`) works while only one enum claims it; the qualified
`Shape.point` always works.

## Match

Consuming an enum must handle every variant. The subject arrives through a pipe, arms chain
with `or`, first match wins:

```toylang
enum Shape { point, circle{r: Int} }

fn area_ish(s: Shape) -> Int = s | circle{r} -> r * r or point -> 0

{a: area_ish Shape.point, b: area_ish circle{r: 3}}
```

```output
{"a":0,"b":9}
```

`circle{r}` binds the payload's field; a unit arm is just the name. Leave a variant out and
the program is refused at compile time, naming what is missing -- unless the chain ends in
`any()`, the explicit way to say "everything else":

```case
enum_match_default
```

The payload can be any single type, not only a record -- `celsius(Int)`, `some(Vec<Int>)` --
and in a bare payload arm `.` becomes the payload itself. The
[guide on enums](../guides/enums.md) works through typing a real wire format with these.

Next: [streams](05-streams.md), where the data comes from stdin.
