# Step 2: JavaScript backend

Same programs, same outputs, through `node` instead of `mlua`.

The easy backend, and it is here to pay for the harness rather than for itself. JavaScript is
dynamically typed, has real closures, and prints JSON natively, so most of the Lua emitter
transfers. The parts that do not are the interesting ones and are worth watching:

- Record key order. The expectation was that `JSON.stringify`'s insertion order would fight
  Lua's sorted keys and one would have to give. The actual answer was that neither should be
  deciding: **the printer is generated from the type**, so keys are known and ordered at emit
  time and nothing is enumerated at runtime. That also fixes a defect the single-backend version
  was hiding, since a Lua table cannot distinguish an empty record from an empty array. Both
  emitters changed, so this step is not a pure addition. See
  [the lowering needs types the checker already computed](../research-log/the-lowering-needs-types-the-checker-already-computed.md).
- Lua indexes from 1 and JavaScript from 0, which touches every generated loop.
- Lua's `local`-is-not-in-scope-until-after rule was a real bug at prototype 1 step 3, and
  JavaScript's `function` hoisting has the opposite behaviour. Emitting forward declarations
  anyway is harmless and keeps one shape for both. See
  [the backend language has rules the checker does not](../research-log/the-backend-language-has-rules-the-checker-does-not.md).

## Acceptance

Every program in the existing tests produces the same output under both backends. Until step 3
that is checked by duplicating a handful of cases; step 3 is what makes it systematic.
