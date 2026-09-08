Board row `module-routing-syntax-build` (gh:167). Implement `@(path)` module-as-function
routing-arm syntax: a matcher arm whose body is `@(route-groups/foo-bar.toy)` dispatches the
subject to that module's entry function instead of an ordinary expression. Background:
`plans/module-routing-research.md` (candidate 3, "convention routing" -- this is the chosen
candidate, with the maintainer's later refinement of `@path` to parenthesized `@(path)` because
it encloses the path expression).

All open design questions are RULED (see the board row for full text; summarized here):

1. **Entry-point convention**: fixed name `handle`. `@(path)` resolves to the module's
   `pub fn handle(...)`. No way to address a different function by name -- that is explicitly
   out of scope, do not build it.
2. **Dispatch strictness**: full strict statically-typed call. `check_module` the target file,
   then unify `handle`'s signature against the arm's subject type and the arm's expected return
   type exactly like any other call site -- no coercion.
3. **Def visibility**: full merge, prelude-style. Widen `Origin` (currently the fixed 2-variant
   `Copy` enum) to `Origin::Module(PathBuf)` -- it is no longer `Copy`, so every
   `HashMap<_, (Origin, bool)>` site needs updating -- and prepend the routed module's `pub` defs
   to the program's def list the same way `prelude::inject` already does.
4. **Enum-variant collision policy**: submodule enum variants are ALWAYS qualified, never
   bare-resolvable from the caller. Do NOT fold them into the shared `variant_owners`
   bare-lookup table the way (3) folds defs -- this asymmetry between defs and enum variants is
   deliberate, not an oversight.

The prerequisite this row was blocked on, `file-visibility-tracking-build` (gh:166, checker
tracks which file a definition came from and enforces per-call-site visibility), is done and
archived on main already -- build on top of it, don't re-derive it.

Concretely:
1. Parse `@(path)` as a new arm-body kind, distinct from an ordinary call expression (see
   candidate 3's writeup for why: it needs routing-specific type errors and must not be
   confused with a normal function application). `@` is free in toylang syntax today.
2. At check time: resolve `path` relative to the referring file, load and `check_module` the
   target file (following whatever `prelude::inject` already does for loading a module), find
   its `pub fn handle`, and unify its signature against the arm's subject type / expected return
   type with no coercion -- a mismatch is an ordinary type error, not a silent failure.
3. Widen `Origin` to `Origin::Module(PathBuf)` and update every `HashMap<_, (Origin, bool)>`
   site the compiler error output points at when it stops compiling.
4. Merge the routed module's `pub` defs into the program's def list (prepend, prelude-style),
   pruned to what's actually reached -- same reachability rule the prelude already follows.
5. Keep submodule enum variants qualified-only: do not add them to `variant_owners`'
   bare-lookup table.
6. A real example `.toy` program with a matcher arm using `@(path)` against a small submodule
   file with a `pub fn handle`, that type-checks and produces correct output when run on at
   least one backend.

Scope this to whichever backend the routing dispatch naturally reaches first (the routing/merge
logic is checker-side, not backend-side, so this is less of a per-backend choice than usual --
say in your summary whether backend emitters needed any changes at all, and why).

Done-gate: the example program compiles and runs correctly, a type mismatch between the arm's
expected type and the target module's `handle` signature is refused with a clear error, a bare
(unqualified) reference to a submodule enum variant is refused, and `just check` passes with no
regressions.
