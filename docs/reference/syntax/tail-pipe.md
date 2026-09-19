# The tail-pipe marker

`lhs |> callee` is the tail-pipeline marker: `callee` -- a sink -- applied to `lhs`. It is
the one way a sink call is written, and it sits only at the outermost position, because a
sink has no result type for anything else to observe. The program's body is a sink-shaped
body when it is `lhs |> callee`; the sink prints each entry as it goes, and the program has
no value.

```toylang
["ada", "bo", "cy"] |> jsonlines
```

```output
"ada"
"bo"
"cy"
```

The callee must be a sink: `jsonlines`, the one sink builtin, or a function whose declared
return type is `Sink`. A `Sink`-returning function's body is the other place `|>` may appear,
so a named sink can be built once and reused:

```toylang
fn emit(ss: Vec<Str>) -> Sink = ss |> jsonlines


["a", "b"] |> emit
```

```output
"a"
"b"
```

`|>` binds looser than everything and is parsed only at the outermost position, so a nested
`|>` is a parse error rather than a larger expression. The restriction is the general sink
rule, not a special case for the marker: a sink is not a value, so it is legal only as the
program's outermost expression or a `Sink`-returning function's body. A callee that is not
a sink is refused:

```toylang
[1, 2] |> length
```

```error
the callee of `|>` must be a sink, such as `jsonlines` or a function that returns Sink; `length` is not one (at byte 10)
```

The direct call form, `jsonlines(v)`, is the other spelling of the same thing; see
[jsonlines](../builtins/jsonlines.md) for what a sink does with its input.
