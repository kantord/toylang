# What toylang is

A compiled, statically typed language derived from the jq family -- a jq dialect that keeps
jq's semantics as its reference while extending and generalizing them, aspiring to be a real
language rather than a study.

- **Data-oriented.** JSON is the native value model, not a library, and data orientation is
  the organizing principle: transformation, selection, and querying of data are how programs
  are structured, not one feature among many.
- **Compiled**, with seven backends (native/LLVM, JavaScript, Lua, jq, Go, Python, Rust) --
  kept as falsifiers of the design rather than as a compatibility promise
  ([ADR 0002](../adr/0002-backends-as-falsifiers.md)).
- **Rust-like syntax**, aiming at a version of Go's simplicity without following it into the
  absurd: where Go's refusals cost too much, ideas come from Rust instead. Enums are the
  first such import.
- **The ambition is general-purpose** -- eventually HTTP servers, GUIs, web frontends, shell
  scripts -- but through a deliberate sequence. The beachhead is CLI data transformation,
  where the dogfood test is concrete: it replaces jq in its author's own shell. Shell
  scripting is the first area to expand into after that. Result-set-oriented tooling (an
  editor whose buffer is a query result) stays on the long horizon, and nearer-term design
  decisions must not foreclose it.

Aspiring to be real puts weight on claims that a study could leave as prose: the performance
thesis (columnar, vectorized, faster than jq's boxed-iterator-per-step ceiling) is a
commitment that will eventually owe benchmarks, and positioning worries like
[recursive descent's cost](../../plans/questions.md#q7-does--promise-depth-first-order-or-only-the-set-of-nodes)
are real product concerns, not rhetorical ones.

The front end is written from scratch
([`plans/prototype_1.md`](../../plans/prototype_1.md)); an earlier plan to fork jaq's front
end and replace only its interpreter was dropped, because jaq's parser and IR encode jq's
surface syntax, which a Rust-like syntax does not survive, and the jq-conformance corpus that
plan leaned on is a non-goal. jq stays a reference for semantics, not a conformance target.

## Two guiding principles

**1. Do not erase boundaries.** When a structural distinction exists, keep it in the type
rather than flattening it away. If a computation crosses a boundary, the crossing is written
down. Corollaries appear throughout: `[...]` is an explicit operator rather than an implicit
coercion, a tuple of streams beats a concatenated stream with a phantom type parameter, and
`Json` is a named type rather than a permissive escape hatch.

**2. Symmetry.** The type-level guarantee and the runtime guarantee must be the *same*
guarantee. If the type system claims two things are distinguishable, the runtime must be able
to distinguish them, and vice versa. A guarantee that exists only at one level is a bug in
the design.

## Values

```
null   true   42   3.14   "text"   [1, 2, 3]   {name: "ada", age: 36}
```

The JSON value forms, with `Str` a real string type rather than an untyped blob.

## Two worked programs

Vision, not documentation: these use features the language does not build yet (`fold {} with`,
`^.`, `on "]q"`). They show the direction, not what runs today.

Shell, counting errors per service, streaming, in constant memory:

```
#!/usr/bin/env toylang
stdin.lines
  | parse_json?                              # skip malformed lines
  | select(.level == "ERROR")
  | fold {} with (acc, e) -> acc[e.service] += 1
  | to_entries | sort_by(-.value) | .[]
  | "\(.key)\t\(.value)"
```

Editor, where the buffer is a query result:

```
fn view(project: Project) -> Vec<Row> =
    [ project.files[]
      | .diagnostics[]
      | select(.severity >= WARN)
      | {file: ^.path, line: .line, text: .msg} ]

on "]q" -> cursor.next()
on "gf" -> open(cursor.get().file, cursor.get().line)
```

There is no buffer, only a reified search.

## Non-goals

- JavaScript semantic compatibility. Prototype chains, `this` binding, coercion, and array
  holes are explicitly not wanted.
- Being jq. Compatibility with a subset of jq's *semantics* is a starting point and a test
  corpus, not a constraint on the finished language.
