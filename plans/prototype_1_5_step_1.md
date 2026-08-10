# Step 1: typed IR

The checker currently returns a type for the program and, separately, a `HashMap<Span, usize>`
recording how many `Vec` layers each field access distributes over. `lower` reads that table to
build an untyped `Ir`. It works because spans happen to be unique per node.

Replace it: the checker emits a typed IR directly, every node carrying the type it was checked
at, and the side table goes away.

## Why this is first

A dynamically typed backend can consume the untyped `Ir`, so neither Lua nor JavaScript forces
the question. LLVM does, immediately and everywhere -- it cannot emit an add without knowing
whether it is adding integers. Doing the refactor now means the JavaScript backend is written
once against the final shape, rather than written against `Ir` and ported a step later.

It is also the cheapest it will ever be. There is one backend to keep working and 57 tests
pinning the behaviour.

## Acceptance

**Zero test changes.** Not "tests updated and passing" -- no snapshot moves and no assertions
edited. The emitted Lua should be byte-identical, because nothing about the language or the
target changed. Any snapshot that does move is either a bug or a decision that was not supposed
to be part of this step.

That criterion is the whole reason to do this as its own commit.

## Shape

Whether `check` and `lower` merge into one pass or stay two passes over a typed tree is open.
Two passes keeps type errors away from lowering concerns; one pass means the types cannot go
stale between them. Worth deciding by writing the `Vec` case of step 6 on paper first, since that
is the node with the most type-dependent lowering.

The `field_depths` map is deleted rather than kept alongside. If it survives this step, the step
did not happen.
