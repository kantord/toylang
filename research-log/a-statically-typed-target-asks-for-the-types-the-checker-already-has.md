---
type: Note
calendar:
  - 2026-08-24
title: A statically typed target asks for the types the checker already has
description: Go needs a declared name for every record and the element type spelled at every level of distribution, which the dynamic backends never asked for, and the checker had computed all of it already.
tags:
  - backends
  - go
  - typed-ir
timestamp: 2026-08-24T00:00:00Z
---

Adding Go as a fifth backend needed nothing new from the front end, and that is the finding. It
asked only for things the checker already knew and the other backends had been allowed to forget.

**Distribution cannot be depth-polymorphic.** Lua, JavaScript and jq each carry one
`tl_field(v, k, depth)` that serves every shape, because the value knows what it is at runtime.
LLVM sidesteps the same problem from the other side, erasing everything into a `tl_vec` with
columns. Go will do neither: `[]User` and `[][]User` are different types and no single function
spans them. So `db.groups[].members[].name` comes out as nested `tlMap` calls with the element
type written in at each level -- which is exactly the shape the dimension model describes, made
literal.

**Every record needs a name.** A record type here is structural, so `{name: Str, age: Int}` and
`{age: Int, name: Str}` are one type however often either is written. Go is nominal and wants a
declaration before a value can exist, so the emitter is the first that has to decide how many
types the program actually has. Getting that wrong would not be a wrong answer at runtime; it
would be two Go structs with no assignment between them, and it would not compile.

Both are the payoff on
[the lowering needs types the checker already computed](the-lowering-needs-types-the-checker-already-computed.md).
That note called the side table carrying field depth a patch over a seam that wanted merging.
Merging it produced the typed IR, and this is the first target that would have been impossible
without it: there is no side table large enough to reconstruct `[][]tlRec2` from an untyped tree.

## Where Go is stricter than the language

Go has no conditional expression, so `then if c else otherwise` becomes a call to a function
literal rather than an operator. A `tlCond(c, a, b)` helper would have been shorter and wrong:
its arguments both evaluate, and one of them may divide by zero.

Go rejects an unused import while accepting an unused function. That asymmetry decides how the
emitter chooses what to include: helpers are read back off the emitted text, where a false
positive costs nothing and a miss would not compile, and imports come from walking the program,
where a false positive is the thing that breaks. The one place text-reading was not enough is
helper-to-helper: `tlAt` and `tlUnwrap` are written in terms of `tlOpt`, and type inference means
the emitted program need never spell it.

And Go rejects a constant that does not fit, which found a real hole. See
[backends can agree and still be wrong](backends-can-agree-and-still-be-wrong.md).

This is the fifth instance of
[the backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md),
and the first where the target's rules are *stricter* than the checker's rather than merely
different -- so the Go build acts as a second opinion on the emitter, free.

The invariant this note explains cost three separate fixes before it held everywhere: [one invariant, three independent construction sites](one-invariant-three-independent-construction-sites.md).
