# Records and functions

A record is a fixed set of named parts, and the names are part of the type:
`{name: "ada", age: 36}` has type `{name: Str, age: Int}`. Reading a part back out is a
projection, `.age`:

```toylang
fn age_of(user: {name: Str, age: Int}) -> Int = user.age

age_of({name: "ada", age: 36})
```

```output
36
```

That fragment is also the first function. `fn name(param: Type) -> Type = body`: one
parameter, one result, one expression as the body, all types declared. Every function is
unary -- and that is not a limitation, because a record is how several things travel as
one:

```toylang
fn area(r: {w: Int, h: Int}) -> Int = r.w * r.h

area({w: 3, h: 4})
```

```output
12
```

When the argument is a record literal, the parens may go, and the call reads as named
arguments:

```toylang
fn area(r: {w: Int, h: Int}) -> Int = r.w * r.h

area {w: 3, h: 4}
```

```output
12
```

Records nest, projections chain, and a record prints its fields in sorted order -- the
written order is not data:

```toylang
{b: 1, a: {inner: "deep"}}
```

```output
{"a":{"inner":"deep"},"b":1}
```

Functions may call each other in any order, and may recurse; declarations are read before
any body is checked.

Next: [pipes, select, and map](03-pipes.md), where data starts to flow.
