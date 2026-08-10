# Step 3: functions

```
fn greet(who: Str) -> Str = "hello " + who

greet("world")
```

This is where the checker starts doing something a dynamic language would not.

## Entry point

A file is zero or more definitions followed by one bare expression, and that expression is the
program. No `main`. Both worked programs in `draft.md` are bare pipelines, and giving them a
wrapper would make the shell-script case read worse than the shell scripts it replaces.

## Adds

Type syntax, so far only `Str`. Parameter binding and scope. Call checking against a declared
signature. The annotation rule from the draft: a named function must annotate its parameter and
its return type, and it is an error if it does not. Nothing infers a signature from a body.

Functions are unary, so there is no argument list and no separator question.

## Negative case

```
greet(42)     # ERROR: expected Str, found Int
```

This is the first test that proves the checker rejects rather than merely accepts. Worth writing
before the positive one.

## Open

Whether a function may be referenced before its definition appears. Taking the whole file's
signatures in one pass before checking any body costs nothing here and avoids an ordering rule
that would have to be explained later. Recorded rather than decided, because recursion is out of
scope until a later prototype and this only matters once it is in.
