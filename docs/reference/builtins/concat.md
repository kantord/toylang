# concat

<!-- @review Coordinator: the oddities inventory flags this name. Every other language's
concat(a, b) is binary; ours is a unary flatten (jq calls this `add`). Renaming costs three
corpus files. Candidates: `flatten` (says what it does), keep `concat` (jq family
resemblance), or wait for Q2 (what `+` means on two Vecs) and fold it into that story.
Edit this note with your call. -->

`concat(vv)`, of type `Vec<Vec<T>> -> Vec<T>`: flattens one level of nesting, keeping order.

```toylang
concat([[1, 2], [3], [4, 5]])
```

```output
[1,2,3,4,5]
```

Exactly one level. Deeper nesting stays where it was:

```toylang
concat([[[1], [2]], [[3]]])
```

```output
[[1],[2],[3]]
```

Joining two `Vec`s is this same operation, spelled as a two-entry outer literal:

```toylang
concat([[1, 2], [3, 4]])
```

```output
[1,2,3,4]
```

Not to be confused with the `+` operator, which concatenates two `Str`s. `concat` is about
dimensions, `+` is about text.
