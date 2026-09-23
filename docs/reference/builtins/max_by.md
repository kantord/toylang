# max_by

`v | max_by(.key)`, of type `Vec<T> -> Opt<T>`: the entry whose projection `.key` is greatest
(gh:177). An empty `Vec` has no maximum, so the result is `Opt<T>` -- the same absence answer
`max` gives (kantord/toylang#140). Ties keep the first such entry, the way a stable maximum
reads.

No total order on `T` is needed, which is the point of the projection: a backend compares by
the projected key, restricted to the same natively-ordered scalars `sort` takes -- `Int`,
`Int64`, [`Str`](../types/str.md), and [`Char`](../types/char.md). Blocking like `max`, so the
subject is a `Vec` only, never a stream.



The projection is the same `map(.name)` machinery already in the checker: `.` is rebound to
each entry, and the projection's type must be one of those four scalars.

Built on the Go, Rust, Lua, JS, Python, and jq backends so far, like [`sort_by`](sort_by.md),
and refused the same way on native; no runnable fragment or corpus case until it has an arm for
it.

jq's own `max_by` returns the last of equal maxima and null for an empty Vec, so the jq arm does
not use it: it pairs each entry with its key once and keeps the first entry whose key is
strictly greater than the running best.