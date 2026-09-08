# Records

`{name: Str, age: Int}`: a fixed set of differently-typed parts addressed by name, where the
names are part of the type. A record answers what it is from its contents alone, which is
what lets a record literal appear anywhere without an annotation:

```toylang
{name: "ada", logins: [1, 2]}
```

```output
{"name":"ada","logins":[1,2]}
```

A record is not a map. The distinction is where the keys are known: a record's field names
are part of its type and therefore known to the compiler, which is what makes each field carry
its own type, lets a `Vec` of records lay out as one column per field, lets the printer
enumerate fields from the type, and makes `.name` a checked access where a missing field is a
compile error rather than a failed lookup. A map -- a type with arbitrary keys whose lookup
yields `Opt` -- is a different thing, worth having for grouped results and genuinely dynamic
keys, and not this one.

Two things the output above shows. Fields print in the order the type declares them: order is
part of a record type, the printer enumerates fields from the type, and input is normalized to
declaration order on read, so the order keys arrive in on stdin is not data and a given value
prints the same on all seven backends. And `{}` is a complete record whose type is its empty
field set.

Order never separates types: `{a: Str, b: Int}` and `{b: Int, a: Str}` are one type, and a
value spelled either way checks wherever that type is wanted. The declared order still
matters -- it is the order a value checked against the type prints in, carried as metadata
rather than identity. Two spellings are two ways of writing one value, so they are also
[equal](../operators/comparison.md): `{a: 1, b: 2} == {b: 2, a: 1}` is true.

The printing guarantee is scoped to a declared spelling, not absolute. A value is rebuilt to
the declared order of whatever type checks it -- a function argument, a return type, a `Vec`
element, a branch -- so two values of one type that pass through the same check print alike.
A record literal that is never checked against a declared spelling keeps the order it was
written in, so two literals of the same type that never meet at a checked position can print
in different orders.

`{name}` for `{name: .name}` is jq's most-used shorthand and is not adopted, for a reason
better than conservatism: it would settle an open question by abbreviation. Narrowing a record
to a subset of its fields is arguably its own operation -- the way `select` narrows a
dimension -- and the language has not decided it
([Q41](../../../plans/questions.md#q41-is-narrowing-a-record-to-a-subset-of-its-fields-an-operation)),
so sugar that quietly implements one answer makes the question harder to ask. The explicit
form is no burden in practice: `{message: .commit.message, name: .commit.committer.name}` has
names that differ from the paths they come from, which is the ordinary case.

A field is read by [projection](../operators/projection.md): `.name` on a record,
`[].name` distributed over a dimension of records.

Records are also how a function takes more than one thing, since a function takes one
argument (see [functions](../syntax/functions.md)); `area {w: 3, h: 4}` passes one record
and reads as named arguments.

As input, a record type is checked field by field, and undeclared fields are ignored rather
than rejected, so a program can read two fields out of a log line without describing the
whole line:

```case
undeclared_input_fields
```

Field names come from data, so they are exempt from name casing: a JSON object is entitled
to a key spelled `Name`.
