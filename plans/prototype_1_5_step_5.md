# Step 5: scalars, functions, and a runtime

`Int`, `Bool`, comparison, `+`, and user functions, natively. Native goes from 1 of 19 corpus
programs to 8, and the snapshot in `tests/backend_llvm.rs` is what proves it.

## Records moved to step 6

This step was planned as "scalars, functions, records". Records cannot be done here, because
**there is no way to build one**. The language has a `Vec` literal and no record literal, since
object construction was deliberately left out of prototype 1, and `{` appears in the parser only
in type position. So the only record value that can exist comes from `input`.

Doing records here would therefore mean parsing JSON inside the compiled binary, to unlock
exactly one corpus program: the other three record programs also need `Vec`. Records and input
move to step 6, where they arrive with `Vec` and unlock four at once.

## The runtime

Step 4 needed nothing but `write`. This step cannot manage that: concatenation allocates,
printing an `Int` needs formatting, comparing a `Str` needs `memcmp`.

Those are written in C, in `runtime/toylang.c`, compiled and linked by the `cc` call the native
build already makes. The emitted IR declares them and calls them. This is what compilers
normally do, and it is where step 6's JSON parser goes.

```c
typedef struct { const char *ptr; int64_t len; } tl_str;

tl_str *tl_concat(const tl_str *a, const tl_str *b);
tl_str *tl_int_to_str(int64_t n);
int64_t tl_str_eq(const tl_str *a, const tl_str *b);
int64_t tl_str_cmp(const tl_str *a, const tl_str *b);
void    tl_print(const tl_str *s);
```

Two consequences worth stating. There is now a second language in the repository. And `cc` stops
being merely the linker and becomes required for correctness, which the Lua backend never needed
because `mlua` vendors its interpreter.

The runtime is embedded in the compiler with `include_str!` rather than read from disk, so a
built `toylang` does not depend on its own source tree being present.

## Representation

- `Int` is `i64`, `Bool` is `i1`.
- `Str` is a pointer to a `tl_str`, so every value in the IR is a pointer or an integer.

Nothing is passed to the runtime by value. A 16-byte struct is passed in registers under the
SysV ABI, but that lowering is the C frontend's job rather than LLVM's, and hand-written IR that
assumes it is guessing. Passing pointers is unambiguous and costs nothing that matters here.

String literals become a private constant for the bytes plus a private constant `tl_str` holding
a pointer and a length. The length excludes the trailing NUL, which exists only so that C code
can debug-print one.

## Allocation

`tl_concat` and `tl_int_to_str` allocate with `malloc` and nothing frees. That is the deliberate
leak from `plans/prototype_1_5.md`, and keeping it inside the runtime keeps it in one visible
place rather than spread through codegen.

## Where the backends will diverge

Integer overflow. Lua 5.4 wraps, JavaScript loses precision near 2^53, and LLVM does whatever
the `add` says. Nothing in the corpus goes near the edge yet, so the harness will not catch it
until something does. In scope to notice, not to fix: the language should say what it promises
rather than each backend inheriting its host's answer.

String ordering is the other one. `<` on `Str` typechecks today. Lua compares bytes, JavaScript
compares UTF-16 code units, and `tl_str_cmp` compares bytes. Those agree on ASCII and not
otherwise.
