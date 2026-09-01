# Opt

`Opt<T>`: a `T` that may be absent. The prelude declares it as an ordinary
[enum](enum.md) -- `enum Opt<T> { some(T), none }` -- so absence is a tag the value carries
in memory, the same as any other enum's variant, not a null pointer standing in for a
missing `T`. Most `Opt`s come from an operation that cannot promise an entry -- a
collapsing index, [`tail`](../builtins/tail.md), a projection through a ragged dimension --
and the constructors `some(x)` and `none` spell one directly.

```toylang
str(collect(range(5))[3]!)
```

```output
3
```

`!` is the one consumer: it insists the value is there, yields the `T`, and if the value is
absent every backend refuses at runtime (see [unwrap](../operators/unwrap.md)).

`Opt` is in the type grammar, so a function may declare one as a parameter or return type,
which is what lets it hand the absence back instead of being forced to insist:

```toylang
fn head(v: Vec<Int>) -> Opt<Int> = v[0]

str(head([1, 2, 3])!)
```

```output
1
```

An unconsumed `Opt` can be the program's result, and absence prints as `null`:

```toylang
[1, 2, 3][9]
```

```output
null
```

Absence is not emptiness. An empty `Vec` that is present prints `[]`; only a missing entry
prints `null`:

```case
opt_holds_an_empty_vec
```

`Opt` nests the same way any other type does -- nothing in the grammar singles out one
level -- so `Opt<Opt<T>>` is exactly as legal as `Opt<T>`, and its two levels are two honest
tags, not the same null collapsed twice: `some(none)` and `none` are different values in
memory. `!` peels exactly one level, so it can tell them apart. Unwrapping a `some(none)`
survives, because the outer tag says the value is there, and yields the inner absence:

```toylang
fn shallow() -> Opt<Opt<Int>> = some(none)

shallow()!
```

```output
null
```

while unwrapping a bare `none` refuses, because there is no outer value to yield:

```toylang
fn shallow() -> Opt<Opt<Int>> = none

shallow()!
```

```refuses
```

Both programs print `null` if the `!` is dropped; the tag in memory is the only thing that
tells them apart, and `!` is what reaches it.

Serialization does not see any of this. Printing an `Opt` is the one place it is
special-cased against the rest of the enum system: every other enum's payload variant is a
single-key wrapper and its unit variant a bare string
([ADR 0009](../../adr/0009-enums-are-json-native-single-key-wrappers.md)), but `Opt`'s
`some(x)` prints as `x` itself and `none` prints as `null` -- no wrapper, and no tag
surviving onto the wire. The rule predates the general one: `Opt` used to be the
value-or-`null` representation in full, and became a tagged-in-memory prelude enum only
later, keeping the old wire form as the one exception ADR 0009 records.

Because printing drops the tag, it drops it at every level a nested `Opt` has. A present
value inside an absent-looking one and a value that is absent all the way down print
identically:

```toylang
fn deep() -> Opt<Opt<Int>> = some(some(7))

fn shallow() -> Opt<Opt<Int>> = some(none)

fn absent() -> Opt<Opt<Int>> = none

{a: deep(), b: shallow(), c: absent()}
```

```output
{"a":7,"b":null,"c":null}
```

`b` and `c` are different values -- the `!` examples above tell them apart -- but nothing
in `{"b":null,"c":null}` says so. That is the cost of the wire form:
it is lossy the same way every type-level distinction in the output already is
([kantord/toylang#62](https://github.com/kantord/toylang/issues/62)), and `Opt` pays it
twice over when it nests.

Rust's `Option<T>` is the same tagged enum, `enum Option<T> { Some(T), None }`, and the two
languages agree deeper than the name: `serde_json`'s default `Serialize` for `Option<T>`
prints `Some(x)` as `x` and `None` as `null`, so `Option<Option<i32>>` collapses the same
way `Opt<Opt<Int>>` does here -- `Some(None)` and `None` both print `null`. The difference
is where that rule lives. In toylang it is the language's own printer, with no other choice
available. In Rust, `Option<T>` has no wire format of its own; `serde` is a library, its
`null`-for-`None` behavior is one derive among others, and a type that wants a different
shape on the wire picks a different one:

```rust
#[derive(serde::Serialize)]
enum Shape {
    Circle { r: i32 },
}
```

which prints the same externally-tagged wrapper toylang's own
[enums](enum.md) do, `{"Circle":{"r":1}}`, because that shape was `serde`'s choice to make,
not `Option`'s.
