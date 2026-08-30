# flatten

`flatten(vv)`, of type `Vec<Vec<T>> -> Vec<T>`: flattens one level of nesting, keeping order.

```toylang
flatten([[1, 2], [3], [4, 5]])
```

```output
[1,2,3,4,5]
```

Exactly one level. Deeper nesting stays where it was:

```toylang
flatten([[[1], [2]], [[3]]])
```

```output
[[1],[2],[3]]
```

Joining a fixed, known count of `Vec`s is `+` instead (see
[arithmetic](../operators/arithmetic.md)); `flatten` is for when the outer `Vec`'s length is
not known at the call site, such as one built by `map`:

```toylang
flatten(range(3) | map([., .]))
```

```output
[0,0,1,1,2,2]
```
