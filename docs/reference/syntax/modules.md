# Modules

`@("path")` runs another file as a function: the module at `path` is loaded, and its `handle`
function is applied to `.`. A module is a file of declarations with no trailing expression,
the same shape [the prelude](../prelude/index.md) has. The module below lives at
`tests/modules/double.toy`:

```toy
pub fn handle(n: Int) -> Int = n * 2
```

```toylang
21 | @("tests/modules/double.toy")
```

```output
42
```

The path is resolved relative to the file the `@(...)` is written in, and a module's own
routes resolve relative to that module in turn. A program given as text rather than a file, as
the docs harness gives these, resolves from the working directory, which is why the paths on
this page start at the repository root.

The entry point is always named `handle`. A module with no `handle` is refused at the route
that asked for it, and `handle` is reachable only this way: never by its bare name, `pub` or
not, since every module has one. The call is as strict as any other. `.` must have exactly the
type `handle` declares for its parameter, with no coercion, and the result has `handle`'s
declared return type:

```toylang
"x" | @("tests/modules/double.toy")
```

```error
module `tests/modules/double.toy`'s `handle` takes Int, but `.` here is Str (at byte 6)
```

Matching is toylang's dispatch, so a matcher arm is where a route usually sits: each arm hands
its payload to a different file. This is the router shape the feature was asked for, with the
modules `tests/modules/celsius.toy` (`pub fn handle(c: Int) -> Int = c + 273`) and
`tests/modules/kelvin.toy` (`pub fn handle(k: Int) -> Int = k`):

```toylang
enum Temp { Celsius(Int), Kelvin(Int) }

fn to_kelvin(t: Temp) -> Int =
  t
  | Celsius -> @("tests/modules/celsius.toy") or
    Kelvin -> @("tests/modules/kelvin.toy")

[to_kelvin(Temp.celsius(27)), to_kelvin(Temp.kelvin(300))]
```

```output
[300,300]
```

## What a module's declarations become

A routed module is merged the way the prelude is: every definition, `pub` or not, joins the
program under its own name, and the checker decides visibility per call site by the file each
definition came from. A `pub` function is callable from the program by its bare name; a
non-`pub` one is a helper for its own file, callable from that module's `handle` and refused
anywhere else. `tests/modules/greet.toy`:

```toy
fn exclaim(s: Str) -> Str = s + "!"

pub fn shout(s: Str) -> Str = exclaim(exclaim(s))

pub fn handle(name: Str) -> Str = exclaim("hello " + name)
```

```toylang
"bob" | @("tests/modules/greet.toy") | shout(.)
```

```output
hello bob!!!
```

```toylang
"bob" | @("tests/modules/greet.toy") | exclaim(.)
```

```error
`exclaim` is not `pub`, so it can only be called from its own file (at byte 39)
```

Enums are the one asymmetry. A module's enum is merged and can be named, but its variants are
only ever reachable qualified, `Shape.circle({r: 2})`; a bare `circle({r: 2})` does not
resolve through the module's declaration. Two modules may therefore share a variant name without the
program having to tell them apart, which is what a set of route handlers written independently
tends to do. `tests/modules/shapes.toy`:

```toy
pub enum Shape { Circle { r: Int }, Square { side: Int } }

pub fn handle(s: Shape) -> Int =
  s | Circle { r } -> r * r * 3 or Square { side } -> side * side
```

```toylang
Shape.circle({ r: 2 }) | @("tests/modules/shapes.toy")
```

```output
12
```

A module the program routes to twice, under two spellings or from two files, is loaded once.
Whatever a program does not reach in a merged module is pruned before any backend sees it, the
same [reachability rule](../prelude/index.md) the prelude gets.
