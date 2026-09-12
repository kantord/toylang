# The record-forming update

`db.color = color` writes a field into a record, producing a new record with that field
(re)bound. Mutation is a value out, not a side effect: the input record is left alone and
the result is a fresh record sharing its unchanged fields, the same way projection and
every other operator produce values.

> **Ratified design, not yet built.** The record-forming update below is the settled design
> for how record-forming updates typecheck and combine, but the compiler does not implement
> it yet. It is documented here so the decision is not lost, not as behavior that already
> runs.

## `=` is `One`-typed

`=` typechecks only when its right-hand side yields exactly one value ([`One<T>`](../types/stream.md)).
Its right-hand side is an ordinary expression, so if it yields several values the whole
update would fork into several results; the `One` requirement makes that hazard a type
error rather than a naming problem. When the right-hand side does fork, bind it to a plain
name first and then assign that name:

    ("red", "blue") as color | db.color = color

## `Vec op Vec` is cartesian

For every binary operator, `Vec op Vec` is cartesian by default -- each value on the left
meets each value on the right -- matching jq 1.8.2's own default. No new builtin is needed
to opt into that behavior:

    [1, 2] * [10, 100]
