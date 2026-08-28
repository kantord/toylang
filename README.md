# toylang

A compiled, statically typed jq dialect. JSON is the native value model, not a library, and
jq's semantics stay the reference while the language extends and generalizes them: Rust-like
syntax, records and enums in the type system, and a compiler that emits real code instead of
walking an AST.

The ambition is general-purpose, but through a deliberate sequence. The beachhead is CLI data
transformation, where the dogfood test is concrete: it replaces jq in its author's own shell.

**Status: exploratory.** No releases, no stability promise. This page shows what runs today;
the design record, including everything still open, is [draft.md](draft.md).

## What it looks like

[`examples/adults.toy`](examples/adults.toy) reads a user list on stdin and prints the names
of the adults:

```
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users | select(.age >= 18) | .[].name

adults(input)
```

```
$ echo '{"users":[{"name":"ada","age":36},{"name":"tim","age":12},{"name":"grace","age":85}]}' | cargo run --quiet -- run examples/adults.toy
["ada","grace"]
```

The pipeline, `select`, and `.name` are jq's; the typed signature and the record type it
declares are not. `.[]` is written rather than assumed because crossing a dimension is a
boundary, and the language does not erase boundaries.

## Enums

Enums are the first deliberate import from Rust: a declared, closed set of variants, and a
match that must handle every one. As data an enum is plain JSON, never an opaque value
([ADR 0009](docs/adr/0009-enums-are-json-native-single-key-wrappers.md)): a unit variant is a
bare string, a payload variant a single-key wrapper like `{"circle":{"r":1}}`, so an enum
types wire data directly.

[`examples/shapes.toy`](examples/shapes.toy):

```
enum Shape { point, circle{r: Int} }

fn area_ish(s: Shape) -> Int = s | circle{r} -> r * r or point -> 0

{a: area_ish(Shape.point), b: area_ish(circle{r: 3})}
```

```
$ cargo run --quiet -- run examples/shapes.toy
{"a":0,"b":9}
```

Match arms chain with `or`; the first that matches wins. The match is closed-world: a
program whose match handles only `circle`,

```
enum Shape { point, circle{r: Int} }

fn area_ish(s: Shape) -> Int = s | circle{r} -> r * r

area_ish(Shape.point)
```

is refused:

```
a match over `Shape` must cover every variant or end in a default; missing `point` (at byte 73)
```

## Seven backends, kept as falsifiers

The same program compiles to Lua, JavaScript, jq, Go, Python, Rust, and native code through
LLVM:

```
$ for b in lua js jq go py rust llvm; do cargo run --quiet -- run examples/shapes.toy $b; done
{"a":0,"b":9}
{"a":0,"b":9}
{"a":0,"b":9}
{"a":0,"b":9}
{"a":0,"b":9}
{"a":0,"b":9}
{"a":0,"b":9}
```

Seven targets are not a compatibility promise, and not all of them will be kept. Each was
admitted because it is structurally unlike the others and therefore falsifies checker rules
the rest satisfy by accident
([ADR 0002](docs/adr/0002-backends-as-falsifiers.md)): jq immediately broke two programs
that three imperative backends had agreed on, and Go's exact constant arithmetic refused an
out-of-range `Int` literal that four backends had printed unwrapped. The test suite enforces
the agreement: every corpus case runs on every backend, and either they all print the same
output or they all refuse the program.

Each backend emits source (or LLVM IR) that the target's own toolchain runs
([ADR 0004](docs/adr/0004-backends-emit-source-run-by-real-toolchains.md)), so running a
given backend needs that toolchain installed. Lua is the default.

## Where things live

- [draft.md](draft.md): the design record, including the open questions. Long on purpose;
  it is the document of record, not this page.
- [CONTEXT.md](CONTEXT.md): the glossary. Words like record, dimension, and projection are
  used precisely, and this is where they are pinned.
- [docs/adr/](docs/adr): decisions, one file each.
- [tests/corpus/](tests/corpus): one YAML file per case, run on every backend; the corpus is
  the spec ([ADR 0003](docs/adr/0003-the-corpus-is-the-spec.md)).
- [research-log/](research-log): what building it taught, one finding per file.
- [plans/](plans): build order for the piece currently underway.

## Running it

`cargo build` produces the compiler; `just test` runs the suite (it needs `cargo-nextest`
and the backend toolchains). `toylang run FILE [backend]` compiles and runs a program,
`toylang emit FILE backend` prints the emitted source, and `toylang build FILE` links a
standalone native binary.
