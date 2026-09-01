# Module routing: a matcher arm dispatching into a submodule file (gh:162)

Research spike for the board row `module-as-function-routing-design` (gh:162). The question is
the maintainer's note, quoted in the issue:

> let's capture the idea of running other modules as functions; for instance imagine an http
> router that uses matcher expressions for matching different route groups; it may map onto
> another module that handles a specific subgroup like `FooBar -> $(../route-groups/foo-bar.toy)`
> or something similar

That note assumes a module system that does not fully exist. This doc first says what "module"
means in toylang today, then surveys how other languages route pattern-matched cases to separate
files, then proposes syntax candidates. No code changes; this is a decide-row input.

## What "module" means in toylang today

A program is one file: declarations (type aliases, enums, functions) followed by one expression
whose value prints. There are no statements, no `main`, no print call
([a program](../docs/reference/syntax/programs.md)). The compiler reads exactly one file
(`src/main.rs` reads one path, then `build`/`run`); nothing reaches for a second file by name.

A second, smaller unit already exists alongside the program: a **module**, which is a file of
declarations with no trailing expression. `prelude.toy` is the one real, checked-in module. The
[DECIDED section in draft.md](../draft.md#decided-a-rudimentary-module-system-one-prelude-file-and-pub)
records the current shape:

- A module is zero or more `[pub] fn` definitions (and enums), nothing else, parsed by
  `parse::parse_module` rather than `parse::parse`.
- `pub fn` marks an export. Every `pub` definition in `prelude.toy` is merged into every
  compiled program, then pruned to what the body can reach.
- There is no import statement, and a program cannot name what it wants from a module or export
  anything for another file to use. `pub` parsed in an ordinary program changes nothing.
- Non-`pub` is not a working privacy boundary. A `pub` prelude function can only call compiler
  builtins and itself, never a private prelude helper, because a non-`pub` definition is not
  merged into any program at all. Real scoping needs the checker to track which file a
  definition came from and enforce visibility per call site, which does not exist yet.

So "module" already has a precise meaning here: a file of fully-typed, fully-declared function
signatures that some program runs, where nothing yet decides which module or which function. The
maintainer's note generalizes exactly the prelude's arrangement: instead of one always-merged
module, a program names other module files and runs them, and a matcher arm is a natural place
for that naming because matching *is* toylang's dispatch mechanism
([the match reference](../docs/reference/operators/match.md)).

The constraint that shapes every candidate below: toylang has no first-class functions and every
signature is declared, so "running a module as a function" must be resolved by the checker at
compile time against a static signature, the way any call is. There is no runtime indirection to
hide behind. Any candidate therefore needs the file-origin tracking and visibility enforcement
that the DECIDED section names as missing.

## How other systems route pattern-matched cases to separate files

Two shapes recur; they differ in what the target of a match is.

**The whole file is the handler.** The match names a location, and the file that lives there is
the unit that runs. Phoenix LiveView routes a path to a module that is the whole page:
`live "/path", MyAppWeb.PageLive` ([Phoenix router docs](https://hexdocs.pm/phoenix/Phoenix.Router.html)).
Next.js is the purest form: the filesystem path *is* the route (`pages/users/[id].tsx`), and the
file's default export is the handler. The pattern (path) and the handler (file) are separate
axes: patterns centralize in a table or a directory tree, handlers live one per file, and the
mapping is declarative data rather than arbitrary control flow.

**The target is a named function inside a module.** The match names a module and one of its
functions. This is the classic MVC router. Phoenix's `Router` maps `get "/users", UserController,
:index` to the `index/2` function in `user_controller.ex`
([controller guide](https://hexdocs.pm/phoenix/controllers.html)). Rails `match '/foo' =>
'foo#bar'` and Express `app.get('/users', require('./routes/users'))` are the same split, with
the handler reached by a `module#action` string or by requiring a module and calling a method.
The module can serve many routes because each route names a different function in it.

The one thing all of them share, and the load-bearing idea for toylang: **the match pattern is
declared separately from the code that handles the match**, and the target is a named module in
a separate file. What varies is only whether the whole file is the unit or a named function
within it is.

jq, toylang's stated inspiration, is the relevant language-side precedent rather than a routing
framework. jq 1.7 added modules: a file of definitions is brought in with `include "lib";` or
`import "lib" as l;`, and its functions are called qualified, `l::fun`
([jq manual](https://jqlang.github.io/jq/manual/#module-system)). jq does not dispatch matcher
arms to files, but it is the precedent in the same lineage for "a file of definitions named from
another file, called by a qualified name." It maps onto the Phoenix "module and function" shape,
not the whole-file shape.

## Syntax candidates

All three keep the matcher's arm shape unchanged on the left and only extend what an arm's body
may be. The subject arrives through the pipe, so the submodule is applied to `.` in every case.
The sketches use an enum of route names and a `router` function; `Request`/`Response` stand in
for whatever wire types the router actually moves.

**Candidate 1: `$(path)`, the whole module is a function (the maintainer's sketch).**

```toylang
enum Route { FooBar, Baz, Qux }

fn router(r: Route) -> Response =
    r | FooBar -> $(route-groups/foo-bar.toy)
      or Baz   -> $(route-groups/baz.toy)
      or Qux   -> $(route-groups/qux.toy)
```

`route-groups/foo-bar.toy` is a module holding one entry by convention:

```toylang
pub fn handle(req: Request) -> Response = ...
```

`$(path)` is an expression: the entry function of that module, applied to the subject. The path
resolves relative to the referring file at compile time; the checker loads the module, reads
`handle`'s signature, and checks it against the subject type and the arm's expected type, the
way it checks any call. This is the whole-file model (Phoenix LiveView, Next.js page). `$` is
free in toylang source today. The cost is an entry-point convention (`handle`): a module with
several exported functions can only be dispatched through the one the convention names.

**Candidate 2: `path::fn`, a named function in a module (Phoenix Controller#action, jq 1.7).**

```toylang
fn router(r: Route) -> Response =
    r | FooBar -> route-groups/foo-bar.toy::handle
      or Baz   -> route-groups/baz.toy::handle
```

The arm names both the file and one exported function, so one module can serve several arms and
no entry convention is needed. This is the most general of the three and the closest to
mainstream routers and to jq's `import ... as l; l::fun`. It costs more syntax: a `::` namespace
separator, and it implies being able to name a module, which brushes against the DECIDED
no-import-statement stance more than the other two.

**Candidate 3: `@path`, a dedicated routing-arm form (convention routing).**

```toylang
fn router(r: Route) -> Response =
    r | FooBar -> @route-groups/foo-bar.toy
      or Baz   -> @route-groups/baz.toy
```

`@path` is a new arm-body kind reserved for module dispatch, distinct from ordinary function
application. The checker treats it specially: the path must resolve to a module whose entry
signature accepts the subject's type and returns what the arm expects, and the path is pinned at
compile time the way a Next.js route is part of the program. Making file-dispatch a first-class
arm form lets the checker give routing-specific errors and keeps `@` from being confused with a
normal call. It is the whole-file model again, but spelled as a routing primitive rather than a
call. `@` is free in toylang today. Its redundancy with Candidate 1 is the argument against it:
if `$(path)` already says "run this module on the subject," a second spelling needs to earn its
own token.

## What all three depend on

All three resolve a second file, read a definition from it, and check that definition against a
call site. That is the machinery the DECIDED section explicitly says does not exist yet: the
checker does not track which file a definition came from and cannot enforce visibility per call
site. Until that exists, no candidate works even for the prelude's own next step (a `pub`
function using a private helper). The candidates are syntax on top of that missing capability,
not a substitute for it. Whatever is chosen, the file-origin/visibility work lands first.

Each also inherits the prelude's reachability rule: only the submodule functions a program
actually reaches get emitted, so an unused `handle` is pruned the way an unused prelude function
is today.

## Open questions

- Entry-point convention (whole-file forms) vs. naming the function (qualified form): which
  matches how a toylang module file is actually written?
- Whether a submodule dispatch target is a full typed function call or something looser; the
  static-signature check is what keeps eight backends agreeing, so it should stay strict.
- Whether "module" should grow aliases or stay declarations-only, and whether a route submodule
  ever needs its own submodules (the http-router example suggests it might, recursively).

## Provenance

Human-authored: the maintainer's note (gh:162) and the synthesis of what the candidates share.
Derived: the DECIDED module section of draft.md and the toylang reference docs; Phoenix, Next.js,
Rails, Express, and jq 1.7 behavior from their cited documentation. Agent-invented: the three
candidate spellings and the framing of the whole-file vs. named-function axis.

Note on scope: the task brief names `plans/module-routing-research.md` as the deliverable and
also carries a generic "do not touch plans/" constraint. The named deliverable path controls
here, matching every other research spike in `plans/`; this is recorded so the conflict is not
lost. No other file in `plans/` is modified.
