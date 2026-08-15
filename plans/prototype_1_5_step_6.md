# Step 6: Vec, records, input, select, field access

The rest of prototype 1, natively, and the step where the interesting decisions are.

## Records and input arrive here

Moved from step 5, because a record cannot be built: the language has no record literal, so the
only record value that exists comes from `input`. That makes records inseparable from parsing
JSON inside the compiled binary, and doing it here unlocks four corpus programs rather than one.

The JSON parser goes in `runtime/toylang.c` alongside the rest of the runtime. It can be
narrower than a general parser, because the checker already knows the exact type the input must
have and the Rust side already rejects anything that does not match before the binary ever runs.
Whether the binary re-validates or trusts a pre-checked shape is open, and it decides whether
`./adults < data.json` works standalone or only under the harness. It should work standalone.

Acceptance is `examples/adults.toy`, built to a binary, reading the same JSON on stdin and
printing `["ada"]` -- agreeing with Lua and JavaScript through the step 3 harness.

## Layout is the real content of this step

A `Vec<T>` needs a representation, and the obvious one is a length and a pointer. The question
that is not obvious, and that the draft cares about more than it cares about this prototype, is
what `Vec<Record>` looks like:

- **array of structs**: one allocation, elements contiguous, `.name` strides across the record
- **struct of arrays**: one allocation per field, `.name` is a contiguous run

The draft's columnar material, Q7's "recursive descent is embarrassingly parallel on a flat
layout", and Q8's vectorizability question all assume the second. Nothing before this step made
the choice concrete, and nothing forces it until a static backend has to emit a `getelementptr`.

Picking array-of-structs here is fine and probably right for a prototype. Picking it *without
noticing* is not, because a good part of the design's performance argument rests on the other
answer. Whichever is chosen, write down what it costs.

## Allocation, and the decision not to make

`select` allocates: it produces a new `Vec` whose length is not known until the predicate has
run. So does string concatenation, from step 5.

Prototype 1.5 should **leak deliberately**. Allocate with `malloc`, never free, and say so. The
alternative is picking between refcounting and tracing here, and that decision belongs with the
mutation model and the copy-on-write thread in the draft, which are the same decision viewed from
another side. A prototype that runs one program and exits loses nothing by leaking, and a
half-built refcount would be exactly the kind of deferred-work-done-badly the plan avoids
elsewhere.

Q14 asks whether a value-layer `select` copies or masks. This step implements the copy, because
that is the simple thing, and it is the step that would produce real evidence for the question
if the mask were tried instead. Worth revisiting once the harness can measure.

## Field access over a Vec

The construct that forced the typed IR. Natively it is a loop that reads one field out of each
element into a fresh `Vec`, with the element type known at compile time. If the typed IR from
step 1 was done right, this needs no lookaside table and no runtime inspection of any value.

That is the check on step 1: if anything here has to ask what a value is at runtime, the typed IR
did not go far enough.
