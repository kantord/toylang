---
type: Lesson
calendar:
  - 2026-08-30
title: A recursive type is where inline codegen needs a name
description: Every backend writes its printers and parsers by expanding a type inline, which works until the type contains itself; the fix is the same in all seven, and the placeholder standing in for the recursion had let each of them answer wrongly rather than fail.
tags:
  - backends
  - enums
  - codegen
timestamp: 2026-08-30T00:00:00Z
---

`enum Json { arr(Vec<Json>), num(Int) }` type-checks (kantord/toylang#76): behind a `Vec` a
self-reference is a heap indirection rather than a layout with no end. Every backend then failed
to run a value of it, and the reason is one line long. A printer is written by expanding the type
inline -- a Vec becomes a join over the element's printer, a record becomes its fields' printers
concatenated -- and expanding `Json` reaches `Vec<Json>` and starts over.

So a recursive type is exactly where type-directed codegen stops being an expansion and becomes a
function. The fix has the same shape in all seven backends: emit `tl_show_Json` once, and let the
nested occurrence be a call to it. That includes the two whose codegen is not a language. On
native the printer is an LLVM function declared before any body so a body can call it; and the
input parser, whose "codegen" is a descriptor string the C runtime walks, needed a token for the
same idea -- `@Json`, resolved against the enums whose descriptors are open around it, because a
string that spells the type out has no more of a bottom than the printer did.

What did *not* need anything: the runtime layouts. An enum is a boxed tag-plus-payload on native
and a struct with one pointer per payload variant in Go, so a value that contains itself was
already representable everywhere.
[Vec of enum falls into the boxed default nobody chose](vec-of-enum-falls-into-the-boxed-default-nobody-chose.md)
is why. The gap was never in how a recursive value is stored, only in the code that walks a type
to write the code that walks the value.

## The placeholder answered instead of failing

`Type::Enum` carried its own variant list, so that a printer or a validator had them in hand with
no registry beside the tree. A self-referential occurrence cannot carry the real list, so it
carries an empty one. Nothing about that is loud: every consumer read an enum with no variants
and produced *something*.

Go panicked looking up a variant index. Rust emitted a match with no arms and failed to compile.
Native refused at runtime, saying `num` is not a variant of `Json`. Lua and Python emitted
printers that ran and produced a type error mid-string. JavaScript and jq printed the right
answer, because neither had anything to reorder or validate for those particular programs.

Seven consumers, one missing list, seven behaviours -- two of them correct. That last part is the
same trap as
[backends can agree and still be wrong](backends-can-agree-and-still-be-wrong.md): a backend
producing the right answer is evidence only when it produced it for the right reason. Here two of
them agreed with the language by not asking the question.

The repair is that the type stopped being the source of truth about itself. The variant list
travels with the program as a registry, `tir::Program::enums`, and everything that needs variants
asks it rather than reading the copy in hand. Reading the copy is now the bug, and it is a bug
that is greppable, which the wrong answer was not.

Two things made converting a backend at a time safe. `Type`'s equality already compared an enum
by name and arguments and not by its variants, so a placeholder and the real thing are the same
type to every `contains` and every lookup. And the registry answers one layer at a time: asking
for `Json`'s variants gives back a payload that is itself a placeholder, so a caller descends
exactly as far as it has reason to and no further.

## Only what is printed needs a printer

The first cut emitted a printer for every recursive enum the program mentioned. Go rejected it:
the printer used `strconv`, the program's own result was a `Str`, and Go rejects an unused import
while accepting an unused function. The list is now the recursive enums reachable from what the
program actually prints -- its result, plus each `jsonlines` element type -- which is smaller,
and which is the honest description of what the functions are for. Another instance of
[the backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md),
this time deciding not what to emit but what not to.
