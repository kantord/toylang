# sort_by

`v | sort_by(.key)`, of type `Vec<T> -> Vec<T>`: `v`'s entries in ascending order by the
scalar the projection `.key` reads off each entry (gh:177). Ties keep their original order, a
stable sort the way jq's `sort_by` reads.

No total order on `T` is needed, which is the point of the projection: a backend orders by the
projected key, restricted to the same natively-ordered scalars `sort` takes -- `Int`, `Int64`,
[`Str`](../types/str.md), and [`Char`](../types/char.md). Blocking like `sort`, so the subject
is a `Vec` only, never a stream.

The projection is the same `map(.name)` machinery already in the checker: `.` is rebound to
each entry, and the projection's type must be one of those four scalars.
