---
type: Note
calendar:
  - 2026-08-10
title: The lowering needs types the checker already computed
description: Field access distributing over a Vec is the first construct whose lowering depends on a type, and passing that through a side table is a patch over a check and lower split that wants merging.
tags:
  - architecture
  - type-checking
  - prototype-1
timestamp: 2026-08-10T00:00:00Z
---

Through step 4 the pipeline was clean: `check` returned a type and `lower` walked the same AST
without needing one. Step 5 broke that on the first construct it added.

Field access distributes over a `Vec`, so `.name` on `Vec<User>` yields `Vec<Str>` and on
`Vec<Vec<User>>` yields `Vec<Vec<Str>>`. The emitted code has to map that many levels deep, and
the number of levels is a property of the type. `lower` does not have types.

Two ways out, and the choice matters more than it looks.

**Test the value at runtime.** Emit a field accessor that asks whether what it received is an
array and maps if so. It works, it is five lines, and it is wrong for this language
specifically: the type system knew the answer at compile time and the runtime would be
rediscovering it. That is principle 2 failing in the small. A design whose whole claim is that
the type-level and runtime guarantees are the same guarantee should not ship a runtime that
re-derives what the types already settled.

**Carry the answer forward.** The checker records a depth per field-access site, keyed by span,
and the lowering reads it. The emitted helper takes the depth as a literal and never inspects
anything:

```lua
local function tl_field(v, k, depth)
  if depth == 0 then return v[k] end
  ...
end
```

The second was taken. But a `HashMap<Span, usize>` threaded from `check` into `lower` is a patch,
not a design. It works because spans happen to be unique per node, and it will need a second
entry the moment another construct's lowering depends on a type -- which is likely to be the
next one, since anything columnar needs to know the element type to lay it out.

The real fix is for the checker to emit a typed IR rather than annotate an untyped AST from the
side, so that types reach the backend by construction instead of by lookup. That is a real
refactor and it did not need doing to finish prototype 1, so it is recorded here undone rather
than half-built.

## The predicted second instance arrived, and it was the printer

This note guessed the next type-dependent lowering would be columnar layout. It was output.

Prototype 1's printer asked the value what it was: if it is a table with something at index 1,
print an array, otherwise print an object with sorted keys. That worked with one backend and
would have disagreed with the second, because **a Lua table cannot answer the question**. An
empty table is an empty array and an empty record at once, so an empty record printed as `[]` in
Lua and would have printed as `{}` in JavaScript. Nothing was wrong with either printer; the
question was unanswerable.

With the typed IR in place the fix was to stop asking. The printer is now generated from the
result type, so a record's keys are known and ordered at emit time and no runtime enumeration
happens anywhere. That also makes the two backends agree by construction rather than by both
implementing the same convention correctly.

It generalises past output. Any construct whose behaviour depends on what a value *is* rather
than on what it does can be settled at compile time in a statically typed language, and settling
it there is the only way three backends can be made to agree without three matching runtime
conventions. A native target makes this compulsory: there is no value to interrogate, because
there is no runtime type information at all.

So the seam this note describes was real, and closing it was worth more than the one construct
that forced it.

This is the same seam as
[the backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md):
there the backend knew something the checker did not, here the checker knows something the
backend needs. Both say the two ends of this compiler are not yet speaking one language.

Step 6b added a third instance: choosing which column a field lives in is a name-to-index lookup
only the type can do. See
[SoA is cheap until something wants a whole element](soa-is-cheap-until-something-wants-a-whole-element.md).
