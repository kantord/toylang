# Opt

`Opt<T>`: a `T` that may be absent. It is produced, never written: `Opt` is not in the type
grammar, so no annotation can spell it, and every `Opt` a program meets came from an
operation that cannot promise an entry -- a collapsing index, [`tail`](../builtins/tail.md),
a projection through a ragged dimension.

```toylang
str(range(5)[3]!)
```

```output
3
```

`!` is the one consumer: it insists the value is there, yields the `T`, and if the value is
absent every backend refuses at runtime (see [unwrap](../operators/unwrap.md)). Since `Opt`
cannot flow through a function signature, it is consumed close to where it was made.

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
