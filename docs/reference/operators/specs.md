# Index specs

Reaching into a `Vec` says what happens to each dimension: keep it (`[]`), narrow it
(`select`), or collapse it (an index). Every dimension crossed gets a spec, which is why
`[]` is written rather than assumed -- crossing a dimension is a boundary, and the language
does not erase boundaries.

An index collapses its dimension to one entry. Zero-based from the front, `-1` is the last:

```toylang
{first: [1, 2, 3][0], last: [1, 2, 3][-1]}
```

```output
{"first":1,"last":3}
```

What comes back is an [Opt](../types/opt.md), because the entry may not be there;
out-of-range is absence, not an error, and `!` is how a program insists otherwise:

```toylang
[1, 2, 3][9]
```

```output
null
```

A slice narrows by position instead of collapsing to one entry. Zero-based like an
index, either bound optional, and out-of-range bounds clamp to the valid range rather
than answering absence the way the collapsing index does:

```toylang
[0, 1, 2, 3, 4][1:3]
```

```output
[1,2]
```

A negative bound counts from the end, and a start at or past the stop is empty:

```toylang
[0, 1, 2, 3, 4][-2:]
```

```output
[3,4]
```

```toylang
[0, 1, 2, 3, 4][9:]
```

```output
[]
```

`[a:]` and `[:b]` leave the other edge at the dimension's boundary. `[:]` is refused:
both edges already at the boundaries is the identity `[]` is, so there is nothing to
say. A slice is a `Vec` result, so it composes with `+` like the other specs do --
dropping entry `i` is `v[0:i] + v[i+1:]`.

```toylang
[1, 2, 3][:]
```

```error
a slice needs at least one bound (at byte 9)
```

`[]` keeps a dimension at full extent, so what follows applies to every entry. A kept
dimension followed by an inner index collapses inside each entry, and a ragged inner
dimension yields `null` where entries are missing:

```case
index_ragged_inner
```

`[]` needs something after it -- a field, an index, `!` -- since a keep spec at the end of
a chain would be the identity and is refused. The most common thing after it is a field:
`[].name` is [projection](projection.md) distributed over the kept dimension.

Indexing a `Stream` does not exist: collapsing consumes to find the entry, and on a stream
that destroys what it passed. `collect` first.
