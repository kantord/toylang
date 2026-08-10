# Step 5: scalars, functions, records

`Int`, `Bool`, comparison, `+`, user functions, and record types, natively. Everything from
prototype 1 except `Vec`, `select` and field access over a collection.

This is where the typed IR earns the refactor. Each of these is a direct LLVM construct once the
type is known and impossible without it: `Int` is `i64`, `Bool` is `i1`, a comparison is `icmp`
with a predicate chosen by the operand type, a function is a function, and a record is a struct.

## Records are the first real layout decision

A record becomes an LLVM struct, which means field order in memory, alignment, and padding. The
checker sorts record fields by name so that two annotations written in different orders are one
type -- that sort now also decides the memory layout, which is a larger consequence than it had
when it was only about type equality.

Whether a record is passed by value or by pointer is the other half, and it is the first place
the mutation model shows up in generated code.

## Where the backends will diverge

Integer overflow. Lua 5.4 wraps, JavaScript silently leaves the integer range near 2^53, and LLVM
does whatever the `add` says. The corpus will find this the moment a test goes near the edge, and
the honest response is for the language to say what it promises rather than for each backend to
inherit its host's answer. Not in scope to fix here; in scope to notice, since the harness exists
precisely to surface it.

`Str` concatenation is the other one, because it is the first operation that has to allocate. See
step 6, which is where allocation gets decided properly.
