# Step 2: JavaScript backend

Same programs, same outputs, through `node` instead of `mlua`.

The easy backend, and it is here to pay for the harness rather than for itself. JavaScript is
dynamically typed, has real closures, and prints JSON natively, so most of the Lua emitter
transfers. The parts that do not are the interesting ones and are worth watching:

- `JSON.stringify` replaces the hand-written `tl_show`, and its key order is insertion order
  rather than sorted. If the two backends disagree on record key order, the harness catches it at
  step 3 and something has to give.
- Lua indexes from 1 and JavaScript from 0, which touches every generated loop.
- Lua's `local`-is-not-in-scope-until-after rule was a real bug at prototype 1 step 3, and
  JavaScript's `function` hoisting has the opposite behaviour. Emitting forward declarations
  anyway is harmless and keeps one shape for both. See
  [the backend language has rules the checker does not](../research-log/the-backend-language-has-rules-the-checker-does-not.md).

## Acceptance

Every program in the existing tests produces the same output under both backends. Until step 3
that is checked by duplicating a handful of cases; step 3 is what makes it systematic.
