# Escalation: the let-bindings bug is not where the brief said it was

The brief for this lane described the issue-150 bug as the let-chain's final expression
swallowing the next top-level statement across a line break, "the same class of bug" as the
#148 fix. That is not what the scratch files show.

## Evidence

- `scratch_let_multi.toy` and `scratch_let_single.toy` both fail with `expected end of
  program, found {` at the `input {x: Int, y: Int}` line, after the function.
- Removing the `input {x: Int, y: Int}` line (a program whose body is just `f(input)`, the
  let-block unchanged) runs correctly end-to-end: the let-chain's final expression already
  stops at the newline, exactly as the #148 same-line rule requires.
- Replacing the let-function with a plain function and keeping the `input {x: Int, y: Int}`
  line fails the same way. The failure has nothing to do with `let`.
- So the failing construct is `input <type>`: a program-body line that declares what stdin
  holds. No version of toylang parses it; `input` is an atom and the trailing record type is
  left over.

## The decision

The brief's definition of done is explicit: "confirm BOTH scratch files run correctly
end-to-end". The scratch files use `input {x: Int, y: Int}` as a typed-input declaration, so
meeting the DoD requires making that construct parse and check. The alternative -- fixing the
let-chain boundary as the brief describes and leaving `input <type>` unsupported -- does not
meet the DoD.

Chosen continuation: implement `input <type>` as a minimal input-type annotation, applied the
same-line way a call argument is. The annotation resolves into the same `ctx.input` cell the
uses of `input` read, so first-use-wins, validation, and all seven backends pick it up
without changes of their own.

## Costs

- Scope: issue-150 is about `let` bindings; the annotation is a separate feature the issue
  did not ask for. It is required only because the lane's own repro files assume it.
- Risk: an annotation that disagrees with a position that expects a type is a loud error
  ("`input` is used as X here and as Y elsewhere"), and a stream/absence/Char/Int64
  annotation is refused the way `input_read` refuses those wire forms. No previously valid
  program changes meaning: `input` followed by a same-line record type or type name was
  already a parse error before this change.
- The annotation is not extended to `inputs` or `lines`; nothing here needs it.
