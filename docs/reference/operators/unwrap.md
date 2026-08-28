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

`!` is the only consumer `Opt` has, and `Opt` cannot be spelled in a signature, so unwraps
cluster near the indexing and `tail` calls that produce them. The alternative to `!` is
leaving the `Opt` alone and letting absence flow to the output as `null`.

Watch the spacing against `==`: `x! == 1` unwraps then compares, while `x!= 1` lexes as
`!=`.
