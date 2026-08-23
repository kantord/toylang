---
type: Note
calendar:
  - 2026-08-11
title: SoA is cheap until something wants a whole element
description: Struct of arrays cost almost nothing in toylang because no operator extracts one element from a Vec, and the single place that does need one turned out to be printing rather than any language feature.
tags:
  - layout
  - backends
  - prototype-1-5
timestamp: 2026-08-11T00:00:00Z
---

A `Vec<Record>` is stored as one array per field. The usual objection to that layout is the
gather: whenever code wants element `i` as a whole record, it has to read every column and
rebuild one, and that undoes the benefit.

In toylang the gather almost never happens, and the reason is a property of the language rather
than of the layout. **Nothing extracts one element from a `Vec`.** There is no indexing operator,
`[]` is the identity, `select` returns a `Vec`, and `.field` returns a column. So the operations
that look like they need an element do not:

- `.name` on a `Vec<User>` is the name column, shared rather than copied. One small header, no
  element work at all.
- `select(.age >= 18)` binds `.` to a *position* rather than a value, so `.age` compiles to
  `ages[i]`. Nothing is materialised, and the loop comes out in the shape that vectorises without
  anything having optimised it.

That second one is the part worth keeping. The vectorisable form is not something a pass
recovers later; it is what falls out of compiling the obvious thing against this layout.

**The exception is output.** Printing a `Vec<Record>` needs every field of one element at once,
because that is what a rendered object is. So there is exactly one gather in the whole native
backend, it is called `tl_rec_from_vec`, and it exists for printing. That is a better outcome
than the general case and a more honest claim than "no gather ever".

The claim that the boundary does not exist has a shelf life. It holds while the language has no
way to name a single element, and the day an indexing operator arrives -- or any operator that
takes a record out of a collection -- the gather becomes ordinary and this stops being free.
Worth knowing which feature buys the cost.

Implementation note that took a bug to learn: the number of columns cannot stand in for "is this
element a record". A record with one field has one column, exactly like a `Vec<Int>`, and the
input parser used the count as the test. `Vec<{name: Str}>` then stored record pointers where
field values belonged and the program read a pointer as a string. The two questions are
genuinely different and both have to be asked.

Related: [the lowering needs types the checker already computed](the-lowering-needs-types-the-checker-already-computed.md),
since choosing a column by field name is another thing only the type knows.
