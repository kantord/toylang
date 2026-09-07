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

The annotation rule is why a signature can stay this complete. Named functions annotate their
parameter and return types; lambdas must not,and cannot:a lambda's type comes from the
position it appears in, so nothing about a lambda is inferred from its body. This is
**bidirectional checking**, which has two modes. *Synthesis* works bottom-up: given an
expression, work out its type, so `42` synthesises `Int`. *Checking* works top-down: given
an expression,and an expected type, verify it fits. Annotations on named functions are the
seeds synthesis starts from; a lambda is always in checking mode, because it only ever
appears somewhere that already knows what it wants. When there is no expected type, it is an
error rather than a guess: a lambda written with nothing to check it against would need the
checker to invent a type from its body, which is where inference becomes unpredictable and
error messages start pointing at the wrong line.

Named functions are required to annotate for two reasons, neither about terseness. The first
is recursion: inferring the type of a recursive function without a declared signature requires
polymorphic-recursion inference, which is undecidable in general,and the standard fix, used by
OCaml and Haskell, is exactly this annotation requirement. The second is that it keeps
checking **local**: every function can be checked knowing only the signatures of what it calls,
never their bodies, so errors point at the mistake rather than at some distant unification
failure,and compilation stays fast. Annotations only appear at named-function boundaries, so
none of this costs terseness where a boundary does not exist.

The rule is not special to lambdas; it is one instance of a class of forms that can only be
checked and never synthesised. `input` and an empty `[]` literal have the same shape; see
[checked-only forms are a class, not a lambda rule](../../../research-log/checked-only-forms-are-a-class-not-a-lambda-rule.md).

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

A real cycle between two or more named functions runs on six of the seven backends. jq is the
exception: its `def` sees only itself and whatever is already defined above it, with no forward
declaration to bridge a cycle, so `toylang build`/`emit` refuses cleanly for that target rather
than emitting jq source that would fail to compile.

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
