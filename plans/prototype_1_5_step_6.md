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

## Layout: struct of arrays

**Decided: struct of arrays.** A `Vec<Record>` is one array per field sharing a length, not an
array of structs.

The reason is not raw speed, it is that it makes the language's own operators cheap. `.name` on a
`Vec<User>` under SoA is the name column: a header pointing at bytes that already exist, no
striding and no copy. Under AoS it is a gather loop. The draft's columnar material, Q7's
"recursive descent is embarrassingly parallel on a flat layout" and Q8's vectorizability all
assume this layout, so choosing AoS here would have quietly cost a chunk of the design's own
argument.

What makes it affordable is a property of the language as it stands: **nothing extracts a single
element from a `Vec`.** There is no `.[i]`, `[]` is the identity, `select` returns a `Vec`, and
`.field` returns a column. So the AoS/SoA boundary -- gathering columns back into one struct --
never has to be crossed, and the usual reason SoA is painful does not apply yet. It will the day
an indexing operator arrives.

`select` therefore compiles to a loop over indices where `.` is not a value but a cursor into the
columns, so `.age >= 18` reads `ages[i]` rather than materialising a record. That is the form
that vectorises, and it arrives by construction rather than as an optimisation.

The one place a whole element is needed turned out to be printing, since rendering an object
requires every field at once. That is one gather in the whole backend, and it exists for output
rather than for any language feature. A better outcome than the general case, and a more honest
claim than "no gather ever".

Ragged nesting is the part SoA does not answer. `Vec<Vec<T>>` is a column of pointers to inner
`Vec` headers, not Arrow-style offsets. Offsets are the eventual answer and the draft already
says so; a pointer column is the cheap version that works and is worth naming as a known
placeholder rather than a design.

## Split in two

**6a: `Vec` of scalars.** Literal, `select`, printing, `Vec` through functions. Unlocks seven
corpus programs. Needs no layout decision at all, since a `Vec<Int>` or `Vec<Str>` is one array
either way.

**6b: records, input, field access.** Where SoA actually applies, and where the JSON parser
lands. Unlocks the remaining four.

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
