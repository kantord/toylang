# Unwrap

Postfix `!` consumes an [Opt](../types/opt.md): it insists the value is there and yields
the bare `T`.

```toylang
str(tail([1, 2, 3])![0]!)
```

```output
2
```

If the value is absent, every backend refuses at runtime; what each says while refusing is
its own business:

```toylang
[1, 2, 3][9]!
```

```refuses
```

`!` peels exactly one level: absence is tagged in memory, so unwrapping an `Opt<Opt<T>>`
that holds `some(none)` succeeds and yields the inner absence, while unwrapping `none`
itself refuses. The alternative to `!` is leaving the `Opt` alone and letting absence flow
to the output as `null`.

Watch the spacing against `==`: `x! == 1` unwraps then compares, while `x!= 1` lexes as
`!=`.
