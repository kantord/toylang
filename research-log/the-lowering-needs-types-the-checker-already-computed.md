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

This is the same seam as
[the backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md):
there the backend knew something the checker did not, here the checker knows something the
backend needs. Both say the two ends of this compiler are not yet speaking one language.
