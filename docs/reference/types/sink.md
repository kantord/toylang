# Sink

The type of an expression that writes rather than produces a value. The one instance today
is [`jsonlines`](../builtins/jsonlines.md), which prints each entry on its own line as it
arrives; its type is `Sink`, and a function may declare `Sink` as its return type so that a
program's output loop can live behind a name:

```toylang
fn out(v: Vec<Int>) -> Sink = jsonlines v

out([1, 2])
```

```output
1
2
```

A sink is second-class the same way a [Stream](stream.md) is, and for the same reason: it is
not a value, so it never sits inside a `Vec`, a record, a `Stream`, or an enum payload, and it
is never a parameter. The checker states the rule once, by position, rather than per builtin:
a `Sink` is legal only as the program's outermost expression or as the body of a
`Sink`-returning function.

```toylang
[jsonlines([1])]
```

```error
a sink is not a value, so it is legal only as the program's outermost expression or a Sink-returning function's body (at byte 1)
```

What stdout and stderr are beyond this one sink, and whether a program writes or returns, is
[Q35](../../../plans/questions.md#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return).
