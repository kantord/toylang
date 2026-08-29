# Functions

`fn name(param: Type) -> Type = body`. A function takes at most one parameter and returns one
result, and its body is one expression. Named functions declare their types fully; nothing
about a signature is inferred.

```toylang
# fmt: syntax-example
fn double(x: Int) -> Int = x * 2

double 21
```

```output
42
```

A function may also take no parameter, written `fn name() -> Type = body` and called
`name()`:

```toylang
fn greeting() -> Str = "hello"

greeting()
```

```output
hello
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

What a signature cannot say: a `Stream` result without a `Stream` parameter (a stream is
born only at a source; see [Stream](../types/stream.md)). A function is not a value -- it
cannot be stored, passed, or returned -- and the nine [builtin names](../builtins/str.md)
cannot be redefined.

Bare application, `f x`, is the default call form for a function that takes one argument;
since a function is never variadic, parens never said which argument is which -- only where
the argument starts and ends -- and `f(x)` is the same call with the argument grouped. Chains
read right-to-left: `str double 21` is `str(double(21))`. Reach for the parens when the bare
form would read differently: `-` starts subtraction rather than an argument (`f -1` is
`f - 1`), and `.` and `[` bind tighter as [projection](../operators/projection.md) and
indexing, so a projection or Vec-literal argument is spelled `map(.name)` or `some([4, 5])`.
An argument must also start on the same line as its function; to call across lines, use the
parens. A nullary function has no bare form -- `name` alone is a reference the checker would
have to disambiguate from a call -- so it is always called `name()`.
