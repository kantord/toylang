# input

`input` is stdin as one JSON value, read whole. It has no type of its own: it is checked
against whatever the position expects, first use wins, so it is normally handed straight to
a function whose parameter type says what stdin must be.

```case
adults
```

Before the program runs, the value is validated against that type -- a parse, not a
coercion. `{"age": "36"}` where `Int` was declared is an error, not a conversion; a number
that does not fit in 32 bits is refused; a missing declared field is refused; undeclared
fields are ignored. See [records as input](../types/record.md) and
[enums as input](../types/enum.md).

Any position that expects a type can supply `input`'s -- an argument, an annotated function
body, a record field checked against a declared record type. First use wins, and it wins for
every later use too: once one position has typed `input`, a use where nothing expects
anything means the same value at the same type.

```case
input_typed_by_first_use
```

A program whose only use of `input` sits where nothing expects anything is still refused
with "cannot tell what `input` contains": there is no first use to borrow from.

`input` reads the same real stdin as [`inputs`](inputs.md) and [`lines`](lines.md), so a
program uses at most one of the three: any two together are refused, because they would
read one resource two different ways.
