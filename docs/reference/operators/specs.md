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
