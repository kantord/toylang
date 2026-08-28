# Records

<!-- @review Coordinator, needs your ratification (issue #24): field order now lives in the
type (your decision), and the implementation took the strict reading -- {a: 1, b: 2} and
{b: 2, a: 1} are now DISTINCT types, so passing one where a function declares the other is
a type error. Disclosed as agent-invented, so it is a placeholder until you rule. The
alternative: order-insensitive typing where declaration order only controls printing, so
both spellings interchange and print the declared way. Strict buys wire-order fidelity and
a simpler checker; loose buys friendlier call sites. Edit this note with your ruling. -->

`{name: Str, age: Int}`: a fixed set of differently-typed parts addressed by name, where the
names are part of the type. A record answers what it is from its contents alone, which is
what lets a record literal appear anywhere without an annotation:

```toylang
{name: "ada", logins: [1, 2]}
```

```output
{"name":"ada","logins":[1,2]}
```

Two things the output above shows. Fields print in the order the type declares them: order is
part of a record type, the printer enumerates fields from the type, and input is normalized to
declaration order on read -- so every value of a type prints identically on all seven
backends, and the order keys arrive in on stdin is not data. And `{}` is a complete record
whose type is its empty field set.

Because order is part of the type, `{a: Str, b: Int}` and `{b: Int, a: Str}` are two types.
Where they meet, the error says the fields agree but their order does not.

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
