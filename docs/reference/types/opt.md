# Opt

`Opt<T>`: a `T` that may be absent. Every `Opt` a value carries came from an operation that
cannot promise an entry -- a collapsing index, [`tail`](../builtins/tail.md), a projection
through a ragged dimension.

```toylang
str(range(5)[3]!)
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
