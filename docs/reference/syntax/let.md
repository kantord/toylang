# let bindings

`let <name> = <expr>` names a value for the rest of a function body. The bindings sit on
their own lines, one after another, and the expression that ends the block is the body's
value -- there is no `in`, no keyword to pair, just a sequence of bindings followed by a
result. The block's value is checked against the function's return type, which is what lets
the final expression read the bound names.

```toylang
fn classify(p: { x: Int, y: Int }) -> Str =
  let m = p.x * p.x + p.y * p.y
  m | m == 0 -> "origin" or m < 100 -> "near" or "far"

classify { x: 3, y: 4 }
```

```output
near
```

A `let` block is only reachable as a function body: the program's own body is one
expression, so a top-level `let` is a parse error rather than a declaration. Inside a body,
bindings stack, and a later binding may read an earlier one:

```toylang
fn f(p: { x: Int, y: Int }) -> Int =
  let a = p.x * 2
  let b = a + p.y
  a + b

f { x: 3, y: 4 }
```

```output
16
```

A later binding may also shadow an earlier one, or the function's own parameter; the name
that wins is the innermost binding in scope:

```toylang
fn f(x: Int) -> Int =
  let x = x + 1
  let x = x * 2
  x

f 5
```

```output
12
```

The form was chosen over `let ... in` for the same reason the language has no statements:
a block that ends in a value needs no closing keyword, and a sequence of bindings followed
by a result is the one shape a body already had. Nothing about a binding is inferred -- the
value's type is synthesized the way any expression's is, and the name is checked against it
at each use.
