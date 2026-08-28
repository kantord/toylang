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

Only positions the checker pushes an expectation into can type `input` -- an argument
position is the reliable one. A record field is synthesized instead, so `{a: input}` fails
with "cannot tell what `input` contains" even when a neighboring use was already typed.

`input` reads the same real stdin as [`inputs`](inputs.md) and [`lines`](lines.md), so a
program uses at most one of the three: any two together are refused, because they would
read one resource two different ways.
