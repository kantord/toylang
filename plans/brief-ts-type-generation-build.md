Board row `ts-type-generation-build` (gh:160). Follow-up split from `js-node-web-split-build`
(archived, gh:160 ruling "one system, all four together"; that dispatch was scoped down to the
Node/Web split slice only, deferring the other three pieces -- this is one of them).

Context: the JS backend (`src/emit_js.rs`, `pub fn emit(program: &Program, target: JsTarget)`)
emits plain `.js` with no accompanying type information -- a consumer importing the emitted
module gets no autocomplete or type checking. toylang's own type system lives in `src/ty.rs`
and is fully resolved by the time codegen runs (every emit backend consumes an already-checked
`Program`), so the information needed to generate accurate `.d.ts` declarations already exists
at the point `emit_js::emit` is called -- it just isn't walked into a second output.

Build TypeScript type generation:
1. Survey `src/ty.rs` for the resolved type representation and `src/emit_js.rs` for how
   top-level declarations (functions, exported values) map to emitted JS names.
2. Add a codegen path that walks the same typed `Program` and produces a `.d.ts` file
   (declaration syntax, not full TS) describing the emitted JS module's shape: exported
   function signatures and value types, using TypeScript's built-in types where they map
   cleanly (number, string, boolean, arrays) and reasonable structural types otherwise.
3. Wire it into the CLI/build so a JS-target compile optionally (or always -- pick the
   simpler mechanism, e.g. a sibling `<output>.d.ts` written alongside `<output>.js`)
   emits the declaration file. Cover both `JsTarget::Node` and `JsTarget::Web` if the
   type surface differs between them; if it doesn't, one generator suffices for both.
4. Keep existing `.js` emission byte-for-byte unchanged -- this is purely additive.

Done-gate: a real example program with a typed function signature compiles to JS via the
existing backend, and a sibling `.d.ts` is emitted whose declared types are correct for at
least int, str, bool, and one aggregate (list or record) parameter/return case -- verified
by an actual `tsc --noEmit` (or equivalent) check against a small TS consumer file that
imports the emitted module and uses it in a type-correct way; `just check` passes with no
regressions on any other backend.
