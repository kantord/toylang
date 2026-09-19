# --explain-offload

Whether a stage of a program can be vectorized is not part of its type. It is derived from
[cardinality](../../guides/cardinality.md): a `map` or `select` over a [Vec](../types/vec.md)
has a known extent, so it compiles to a loop over an already-materialized dimension, which is
what a kernel is; over a [Stream](../types/stream.md) the extent is unbounded and entries
arrive one at a time, so the same stage runs per element and no kernel is possible. The design
record states the predicate under
[Cardinality is the kernel-admissibility predicate](../../../draft.md#cardinality-is-the-kernel-admissibility-predicate),
and the choice to keep it out of signatures is the
[vectorizability question](../../../plans/questions.md#q8-is-vectorizability-visible-in-the-type-system-or-a-silent-optimization).
Keeping it silent costs discoverability, and this flag is the answer to that cost: an opt-in
report of which stages became kernels and which fell back, and why.

The flag goes first, before `run`, `emit`, or `build`; anywhere else it is a usage error. The
report goes to stderr, one line per decision, so the command's own output on stdout is
untouched. The first line is the program-level fusion decision, then one line per `map` or
`select` in source order, the functions' bodies before the program body.

```toylang
[1, 2, 3] | select(. >= 2)
```

```output
[2,3]
```

```
$ toylang --explain-offload run filter.toy
no `jsonlines` sink: nothing to fuse
select over Vec<Int>: became a compaction kernel (Opt<Int>: zero or one kept per entry, known extent, vectorizable)
[2,3]
```

The same `select` over a stream is the other verdict. These two programs keep the same adults
and differ only in what the function takes:

```toylang
fn adults(db: { users: Vec<{ name: Str, age: Int }> }) -> Vec<Str> =
  db.users | select(.age >= 18) | .[].name

adults(parse(stdin))
```

```input
{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": 4}]}
```

```output
["ada"]
```

```
$ toylang --explain-offload emit vec_adults.toy lua > /dev/null
no `jsonlines` sink: nothing to fuse
select over Vec<{name: Str, age: Int}>: became a compaction kernel (Opt<{name: Str, age: Int}>: zero or one kept per entry, known extent, vectorizable)
```

```toylang
fn adults(
  users: Stream<{ name: Str, age: Int }>
) -> Stream<{ name: Str }> =
  users | select(.age >= 18) | map({ name: .name })

jsonlines(adults(stdin | map(parse(.))))
```

```input
{"name": "ada", "age": 36}
{"name": "bo", "age": 4}
```

```output
{"name":"ada"}
```

```
$ toylang --explain-offload emit stream_adults.toy lua > /dev/null
the `jsonlines` pipeline fused into a read-one/transform-one/write-one loop over `inputs`
select over Stream<{name: Str, age: Int}>: fell back to per-element streaming (Stream's extent is unbounded and unknown, so no vectorizable kernel is possible)
map over Stream<{name: Str, age: Int}>: fell back to per-element streaming (Stream's extent is unbounded and unknown, so no vectorizable kernel is possible)
```

The first line changed too. A `jsonlines` sink over a stream fuses into one loop that reads,
transforms, and writes an entry at a time, and the report names the source it reads from:
`inputs` for `stdin | map(parse(.))`, `lines` for a raw `stdin`, `range` for a
[`range`](../builtins/range.md). Over a Vec the sink has nothing to stream and the report says
so, with the map that fed it counted as a kernel:

```toylang
fn names(db: Vec<{ name: Str, age: Int }>) -> Vec<Str> =
  db | map(.name)

jsonlines(names(parse(stdin)))
```

```input
[{"name": "ada", "age": 36}, {"name": "bo", "age": 4}]
```

```output
"ada"
"bo"
```

```
$ toylang --explain-offload emit vecmap.toy lua > /dev/null
`jsonlines` ran eagerly over a materialized Vec<Str>: known extent, so the sink had nothing to stream
map over Vec<{name: Str, age: Int}>: became an elementwise map kernel (One<{name: Str, age: Int}>: exactly one output per input, known extent, vectorizable)
```

A program with no `map` or `select` gets the fusion line alone. A program that does not
compile gets no report: the compile error is printed as usual and nothing else. In front of
`fmt` the flag is a usage error, since a formatter run has nothing to explain.
