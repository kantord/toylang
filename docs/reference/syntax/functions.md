# Functions

`fn name(param: Type) -> Type = body`. A function is unary -- one parameter, one result --
and its body is one expression. Named functions declare their types fully; nothing about a
signature is inferred.

```toylang
fn double(x: Int) -> Int = x * 2

double(21)
```

```output
42
```

Several things travel as one record, and a record-literal argument may drop its parens, so
`area {w: 3, h: 4}` reads as named arguments:

```case
call_without_parens
```

Functions can call forward and can recurse; signatures are collected before any body is
checked:

```case
forward_reference
```

What a signature cannot say: `Opt` (not in the type grammar), or a `Stream` result without
a `Stream` parameter (a stream is born only at a source; see
[Stream](../types/stream.md)). A function is not a value -- it cannot be stored, passed, or
returned -- and the nine [builtin names](../builtins/str.md) cannot be redefined.

There is also a bare application form, `f x`, legal only where an expression begins fresh.
No real program in the repository uses it; write `f(x)` or `f {record}`.
