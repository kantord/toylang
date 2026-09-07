# The prelude

`prelude.toy` is one checked-in toylang file whose `pub` definitions are always available
to every program: they are merged in whole, with no import statement,and no way for a
program to name what it wants or to export anything of its own for another file. `pub`
marks a `fn` -- or, in `prelude.toy`,an `enum` -- as part of that always-available set.
`pub fn` is parsed and stored wherever a `fn` is,including in an ordinary program,but it
has no effect there yet, because nothing imports from a program file today.

`prelude.toy` is parsed as a module,not as a program: `parse::parse_module` is a second
entry point next to `parse::parse`,anda module is zero or more `[pub] fn` or `[pub] enum`
declarationsand nothing else -- no body expression to fake. It currently holds
[`join`](join.md)and [`join_lines`](join_lines.md),andthe [`Opt`](../types/opt.md)and
[`Result`](../types/result.md)enums. A non-`pub` definition is a helper for its own
file: a private one stays in so it can serve as a helper for a `pub` one,andthe
checker refuses calls to it from any other file,at each call site,by file origin. `join`
and `join_lines` share the private helper `join_parts` this way.

Merging every `pub` definition unconditionally means an unused prelude function would sit in
every compiled program's `Program.funcs`,which is what `tags::node_types` walks and what
every backend turns into output. `check::check` prunes `Program.funcs` to whatever the
program's body can actually reach,directly or through a called function's own calls --the
same treatment an unused function the program wrote itself now also gets.


## What is not here yet

Imports,a way for a program's file to export for another,and more
than one file to import from -- are tracked as
[Q36](../../../plans/questions.md#q36-does-a-real-module-system-need-imports-multiple-files-and-enforced-privacy).