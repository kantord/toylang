# Step 5: typed input

```
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users[] | select(.age >= 18) | .name

adults(input)
```

Reading `{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": 9}]}` on stdin and printing
`["ada"]`.

## Adds

Record types in the type syntax, `Vec<T>` with a nested element type, field access, and the stdin
path with `serde_json`.

## The input has to be parsed, not trusted

The declared parameter type is a static claim about data that arrives at runtime, so something
has to check it. The draft answers this for shape under the projection discussion: rectangularity
is earned by a named operation that can fail, rather than assumed. The same applies here, and the
cheapest honest version is to validate the whole input against the declared type at startup and
exit with an error naming the path that did not match.

What this must not become is a coercion. `{"age": "36"}` is an error, not an `Int`. The moment
input validation coerces, the type stops describing the value and principle 2 is broken at the
one place where the runtime and the type system actually meet.

`input` as the name of the stdin value is a placeholder. The draft writes `stdin.lines` for the
streaming case, which is a different thing and belongs to whichever prototype introduces the
effect layer.

## Negative cases

```
db.nmae                # ERROR: no field `nmae` on {users: ...}
```

This is the payoff. It is the first thing in the prototype that jq cannot structurally do, and it
is worth being the commit that closes prototype 1.

Plus the runtime one: input that does not match the declared type exits with an error rather than
producing `null`.

## Open

Whether field access desugars to a lens here or stays a getter. The draft says a lens, because
that is what makes `|=`, deletion and path enumeration share one syntax. None of those are in
prototype 1, so a getter is enough and the lens is not yet earning anything. Deferred undone
rather than half-built: no lens type that nothing reads.
