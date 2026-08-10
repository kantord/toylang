# Prototype 1.5

Three backends for the language prototype 1 already has. **No new language features.** The
grammar is frozen at exactly what `plans/prototype_1.md` describes, so anything that breaks is a
backend problem and nothing else.

## Why now rather than later

**Q5 is absent, not deferred.** The stream-lowering question is recorded as blocking all backend
work, and it does block it -- once there are streams. Prototype 1 has no effect layer, so
everything has statically known extent and lowers to a counted loop on any target. This is the
only window in which three backends are cheap, and it closes the moment streaming input arrives.

The payoff is the order things get decided in. Attacking Q5 with a native backend already in hand
means the answer has to satisfy a real static target. Attacking it with only Lua means designing
stream lowering around coroutines and discovering afterwards that native cannot have them, which
is the specific trap the question exists to warn about.

**Disagreement becomes expressible.** With one backend there is no such thing as the backends
disagreeing, so the corpus that would have come free with jaq's 640 assertions has had nothing to
check. See
[losing jaq's corpus means building the agreement harness](../research-log/losing-jaqs-corpus-means-building-the-agreement-harness.md).

**The typed IR stops being optional.** Lua and JavaScript are dynamically typed and will consume
the current untyped `Ir` unchanged. LLVM will not: it has to know whether a value is an `i64`, a
pointer, or a string, and what a `Vec<Int>` is in memory. The side table currently carrying field
depth from the checker into the lowering is a patch, and a static backend is what turns it into a
blocker. See
[the lowering needs types the checker already computed](../research-log/the-lowering-needs-types-the-checker-already-computed.md).

## Two halves

**1.5a** is the typed IR, the JavaScript backend, and the agreement harness. Three backends'
worth of structure paid for by the easy backend. It ships something whole on its own.

**1.5b** is LLVM. It is much larger than 1.5a and it is where the value representation and
memory management get decided, so it is separated deliberately: if it stalls, 1.5a still landed.

1. [Typed IR](prototype_1_5_step_1.md) -- a pure refactor, zero test changes
2. [JavaScript backend](prototype_1_5_step_2.md)
3. [Agreement harness](prototype_1_5_step_3.md)
4. [LLVM skeleton](prototype_1_5_step_4.md) -- a native binary that prints
5. [Scalars, functions, records](prototype_1_5_step_5.md)
6. [Vec, select, field access](prototype_1_5_step_6.md) -- layout and allocation

## Toolchain

Verified on this machine rather than assumed: inkwell 0.10.0 supports LLVM 22.1, and LLVM 22.1.8
is installed with `libLLVM-22.so`, so the dependency is
`inkwell = { version = "0.10", features = ["llvm22-1"] }`.

LLVM emits an object file and does not link it, so a native build still shells out to `cc`.
JavaScript runs through `node` as a subprocess. Both add a toolchain requirement that the Lua
backend does not have, since `mlua` vendors its interpreter. That is accepted rather than worked
around: step 6 produces a binary that has to be executed as a subprocess regardless, so the
harness needs to run subprocesses either way.

## What 1.5 does not answer

Q5, still. Nothing here streams. Also Q2, Q7 and Q8, though step 6 is the first place the
columnar question becomes concrete, because choosing a layout for `Vec<T>` is choosing whether a
`Vec` of records is an array of structs or a struct of arrays.
