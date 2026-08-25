---
type: Lesson
calendar:
  - 2026-08-26
title: One invariant, three independent construction sites
description: A Vec of records is one column per field on native, and three unrelated places that build one each needed their own fix for it -- field access, map, and a Vec literal -- because none of them shared code and none of them implied the others were correct.
tags:
  - native
  - llvm
  - struct-of-arrays
  - correctness
timestamp: 2026-08-26T00:00:00Z
---

Native's struct-of-arrays layout has one invariant: a `Vec` of records is one column per field,
never a column of record pointers. Three separate places have now violated it, each found by a
program nothing before it had written, and none of the first two fixes implied the third was
safe.

**Field access**, fixed in commit `363710f`. `db[].commit.message` on a `Vec<{commit: {...}}>`
gave empty strings, because reading a record-valued field off a `Vec` of records returned the
shared column directly instead of spreading it back into one column per subfield. Found by
pointing the language at real GitHub API data -- the first program with a record nested inside a
record that a `Vec` had ever been asked to reach through.

**`map`**, fixed alongside product literals in `a4b2198`. `tl_map_new` allocated one column
unconditionally, so a `map` whose body built a record stored the whole record pointer where
component values belonged. Predicted in writing before it was hit -- the design note for record
literals named `map {a: {b: .x}}` as the shape that would break it, and it did, on the first try.

**A `Vec` literal**, fixed today. `vec_lit`'s write loop always wrote to column 0, so
`[{a: [1,2], b: "x"}, ...]` -- a literal built directly from record literals, rather than a
`Vec` arriving from `input` or produced by `map` -- stored the first record's whole pointer in
column 0 and left every other column uninitialized. `jsonlines`, built to reproduce the jq
tutorial's fourth step, was the first thing to call a record-typed `Vec` literal with more than
one element; nothing in 89 corpus programs had.

None of these three fixes could have prevented the next one. They are unrelated code paths --
`field_of`, `map`'s loop, `vec_lit`'s loop -- that each independently decide how to write a
`Vec`'s columns, and fixing one says nothing about whether the other two got it right. The
general form: **an invariant that spans several independent implementations of the same idea
needs checking at every implementation, not once and then trusted.** A shared helper that all
three called would have made this three bugs found once rather than three bugs found three
times; none of them share one today.

This is the same shape as
[backends can agree and still be wrong](backends-can-agree-and-still-be-wrong.md), one level
down: there, four *backends* agreed by coincidence because none of them had been asked the right
question. Here, three *code paths within one backend* each got a chance to violate the same
invariant independently, and did, on the first occasion each was actually exercised.

See
[a statically typed target asks for the types the checker already has](a-statically-typed-target-asks-for-the-types-the-checker-already-has.md)
for why native needs this invariant at all -- it is the price of having no runtime type
information to fall back on, which every other backend is spared.
