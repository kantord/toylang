# Type flow: the checker learns to read annotations top-down

The deepest checker change on the books, ratified in oddities round two. Today every
expression is synthesised in isolation and compared afterwards, so a declared type is never
used to resolve what sits inside it. Three refused programs define the goal:

```
fn nothing(x: Int) -> Vec<Int> = []          -- cannot tell what `[]` contains
fn initial() -> Status = "Active"            -- expected Status, found Str
{a: f(input), b: input}                      -- second `input` unresolvable
```

After the rework all three compile as written: the position's expected type flows into the
expression. Zero new syntax.

## Shape of the change

`expect(ctx, expr, want)` already exists and already handles a handful of forms; the rework
is promoting it from a special case to the checker's other half, with `synth` falling back
where no expectation exists. The order of conquest, each step landing green:

1. **Function bodies.** The declared return type flows into the body expression. This alone
   fixes `[]`-under-annotation and unblocks enum-literal ascription.
2. **Record literals.** A record checked against a record type pushes each field's expected
   type into its value -- fixes the `input`-in-a-field hole, and composes with step 1.
3. **Call arguments.** A call against a known signature pushes the parameter type into the
   argument. This is the step that revives the rejected `parse` design and unblocks the
   stdin redesign (its decide row waits on this plan's completion).
4. **Map/select bodies.** Expected element types flow through mapper bodies -- the original
   blocker the `inputs` decision documented. Careful with the subject-rebinding machinery;
   the stream typings ride on it.
5. **Conditionals and match chains.** Both branches/arms receive the expectation; the
   hybrid-totality rules for chains must keep working unchanged.

## What must not change

Every currently-compiling program keeps its type exactly (expectation only resolves what
synthesis refused; it never overrides a successful synthesis -- mismatches stay errors, not
coercions). Every current rejection snapshot either stays identical or improves its message;
a rejection that starts compiling must trace to one of the three goal programs' patterns or
it is a bug. The checked-only-forms research-log entry gets its closing update.

## Tests

The three goal programs become corpus/step cases. Each step adds its own: empty Vec in
return position, enum string ascription (unit and wrapper payload), input in record fields,
parse-shaped map bodies, expectation flowing through both conditional branches. The
blindness rule as always: type-level behavior needs step tests, not corpus output.

## Out of scope

The stdin redesign itself (waits for its own session on this foundation), bidirectional
inference beyond declared annotations (no guessing, no unification variables leaking into
errors), and any surface-syntax change.
