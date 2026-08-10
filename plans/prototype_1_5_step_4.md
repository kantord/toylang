# Step 4: LLVM skeleton

```
"hello world"
```

as a native binary. The same walking-skeleton move as prototype 1 step 1, and for the same
reason: prove the whole path before there is anything interesting on it.

`inkwell = { version = "0.10", features = ["llvm22-1"] }`, verified against the LLVM 22.1.8 on
this machine rather than assumed.

## The path is longer than it looks

Lua and JavaScript end at a string of source. LLVM ends at an object file, which is not a
program. The full path is typed IR, LLVM IR, object file, `cc` to link, then execute the binary
as a subprocess. Every one of those can fail differently and the skeleton exists to see all of
them work once.

Keep the LLVM IR dumpable. `toylang emit --target=llvm` is the equivalent of reading the
generated Lua, and it is how the next two steps get debugged.

## What one string literal already forces

A decision about what a `Str` *is*. Lua and JavaScript both have a string type and the question
never came up; here it has to be answered before anything prints.

The cheapest honest answer is a length and a pointer to bytes rather than a null-terminated
`char*`, because the language's strings can contain a null byte and its `length` is not `strlen`.
Printing then goes through `fwrite` rather than `puts`. Taking the cheap C answer here would have
to be undone the first time a string round-trips through input.

Deliberately not decided yet: whether strings are owned, refcounted, or interned. Nothing at this
step outlives the expression that made it.

## Acceptance

`toylang build examples/hello.toy` produces a binary that prints `hello world`, and the corpus
from step 3 runs it as a third backend for the programs it can already handle.
