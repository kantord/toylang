# Native/LLVM backend ergonomics: inkwell layer and a Rust runtime?

Research lane (board row `native-backend-rust-ergonomics-research`, maintainer note 2026-09-04). The brief asks whether the native backend should lean on well-established Rust libraries rather than hand-writing raw LLVM IR/C, ideally writing integrated code directly in Rust or importing from other crates, instead of manually authoring runtime/toylang.c and src/emit_llvm.rs. What follows is what the two hand-written surfaces actually are, what the friction the note cites cost, and what replacing either half would buy and cost. The design round this feeds is composed at the bottom.

## What the backend is made of today

The native backend has two hand-written surfaces, and neither is what the brief's framing assumes:

**The LLVM binding is already inkwell, entirely.** `emit_llvm.rs` is 2808 lines, and every LLVM operation goes through inkwell's `Context`, `Module`, `Builder`, `Target`/`TargetMachine`, and value/type handles (imports at [emit_llvm.rs:13-24](emit_llvm.rs)). There is no raw-LLVM-C-API layer under it to replace. [Q15 (LLVM via inkwell)](questions.md#q15-backend-llvm-via-inkwell-cranelift-or-both) already settled this back-end choice. So the "inkwell-based codegen layer" half of the survey question is the status quo, not a proposal to prototype. What is hand-written on top of inkwell is the IR *construction*: ~2800 lines of `build_call`, `build_store`, `build_int_add` calls expressing the TIR tree in LLVM IR.

 **The runtime is one hand-written C file.** `runtime/toylang.c` is 1602 lines (plus the 2808 of IR). The emitted object file does not contain it: [lib.rs:347-368](lib.rs) shells out to one `cc` call that compiles the object file together with the C source, which is `include_str!`'d out of the repo at [emit_llvm.rs:34](emit_llvm.rs). The comment there and in lib.rs states why: "compiled alongside rather than shipped as a library, which keeps the build to one `cc` call and means there is nothing to install."

So the survey's real question is two narrower ones: is hand-written IR on top of inkwell the right ceiling, and should the C runtime become Rust. A third question hides in the brief's "import functionality from other crates": only a Rust runtime could be linked into the native binary, because every other backend emits source in another language. I'll treat that as part of the runtime question.



## The cost of one more runtime function

Every runtime function the native backend calls is declared in three places that must stay in sync:

1. the C definition (`runtime/toylang.c`);
2. a `Runtime` struct field ([emit_llvm.rs:40-88](emit_llvm.rs), 47 of them);
3. a `module.add_function` call naming the function and re-stating its full LLVM signature ([emit_llvm.rs:137-320](emit_llvm.rs), 47 of them).

Take one existing function, `tl_vec_sort_int`, and what adding it needed:

```c
tl_vec *tl_vec_sort_int(const tl_vec *v) {
    tl_vec *out = tl_vec_new(v->len, 1);
    if (v->len > 0) {
        memcpy(out->cols[0], v->cols[0], (size_t)v->len * sizeof(int64_t));
        qsort(out->cols[0], (size_t)v->len, sizeof(int64_t), tl_cmp_int64);
    }
    return out;
}
```
([runtime/toylang.c:1385](runtime/toylang.c))

```rust
vec_sort_int: FunctionValue<'ctx>,
```
([emit_llvm.rs:83](emit_llvm.rs))

```rust
vec_sort_int: module.add_function(
    "tl_vec_sort_int",
    ptr.fn_type(&[ptr.into()], false),
    None,
),
```
([emit_llvm.rs:299-303](emit_llvm.rs))

The struct field carries no signature;the name and signature live in the field *name* and the `add_function` call,so a rename or a signature change must land identically in C and IR separately, and the C side is never type-checked against either. A signature mismatch between C and the LLVM declaration is a link error at best and a silent ABI mismatch at worst (arguably the safest failure mode is "cc fails", which still only happens when something actually calls the function).

The issue-177 lane is the concrete instance the maintainer note cites. `sort_by`/`max_by` TIR kinds and checker plumbing landed first ([sort-by-max-by-tir](https://github.com/kantord/toylang/commit/15e440e);the C helpers are still to come -- [emit_llvm.rs:1389-1391](emit_llvm.rs) returns `unsupported("sort_by/max_by emission lands in a later step")` today. The lane needed four keyed C helpers (`tl_cmp_keyed_int`, `tl_cmp_keyed_str`, `tl_vec_sort_by_key_int`, `tl_vec_sort_by_key_str`, per the incident log), each carrying the three-way sync above. Its incident log ([plans/incidents/issue-177-20260903/](incidents/issue-177-20260903/)) records 16 separate edits to `runtime/toylang.c`, 17 tool errors (mostly `oldString`-not-found edit failures),and a self-introduced combining-character corruption (the `if #v ==  ̈0` incident) that the worker spent steps detecting and repairing. That is the friction the note is asking about, documented as it happened:the edit failures came from the C file not matching the worker's model of it, and the corruption came from typing into it. Neither failure mode has an analogue in the checker-typed Rust surfaces the other backends patch the same way.



##What a Rust runtime would actually buy

A Rust runtime crate (`extern "C"` + `#[no_mangle]` fns, compiled at compiler-build time into a static archive the `cc` link already does against)would change the native runtime three ways:

**The signature would live once.** The extern block can be generated from one table (or one `#[no_mangle] pub extern "C" fn` per helper),collapsing sites 1 and 2 of the three-way sync into one. Site 3 (the LLVM `add_function`) still needs the signature -- IR cannot be typed from Rust -- but it can be generated from the same table the externs come from. The C side, which is the only side never checked against anything, disappears entirely.

 **The runtime's internals would be checked.** The sort_by/max_by helpers are exactly where the untyped C cost bites: a keyed comparator takes `const void *` and dereferences through casts ([runtime/toylang.c:1370-1380](runtime/toylang.c));the worker hand-patching that surface corrupted it. In Rust, the same code is a typed `sort_by_key` over an index `Vec` followed by a column permute -- `std` supplies the stable sort, the projection is checked, and there is no `qsort` comparator ABI to get wrong. The correctness win is real but not magic: the struct-of-arrays layout ([runtime/toylang.c:194-207](runtime/toylang.c)) means a keyed sort must still project keys, sort an index vector, and permute every column together -- Rust does not erase that structural work, it erases the pointer arithmetic and the unchecked comparator castste.

 **The "integrated Rust" dream is narrower than the brief hopes.** Only the native backend's runtime could link a crate;every other backend emits source in its own target language and cannot import Rust. Concrete instance:the shortest-round-trip float formatter now exists in three copies -- C (`tl_float_to_str`), jq ([emit_jq.rs:80-95](emit_jq.rs)), and Go ([emit_go.rs:254-...](emit_go.rs)) -- because each emitted program must carry its own. A Rust runtime would turn the C copy into Rust, but not de-duplicate across backends; the duplication is inherent to multi-target codegen, not to hand-writing C. Similarly, the Rust source backend ([emit_rs.rs](emit_rs.rs)) already emits self-contained Rust with no external crates, one `rustc` call, for the same "nothing to install" reason ([lib.rs:370-396](lib.rs));having it link the native runtime crate would break that constraint and is a separate tradeoff the round could weigh, not a side effect of the native backend moving to Rust. What Rust actually buys here is internal to the native backend:the runtime shares type/utility code with the compiler crate (a `tl_str` could be a struct in `src/`, the float formatter could live in one Rust fn instead of one C fn),and the leak-everything memory posture the C file declares deliberately ([runtime/toylang.c:7-10](runtime/toylang.c),"Prototype 1.5 leaks deliberately") would have to be re-expressed in Rust (`Box::leak` or a global arena),which is a design decision the mutation-model work owns, not a mechanical translation. That is the one place moving to Rust costs a *decision* rather than a build step.



##What it would cost

- **Per-program toolchain stays `cc`.** A Rust runtime compiled at compiler-build time into a static archive links the same way toylang.c links today:one `cc` call with the object file plus the archive. Nothing to install stays true;nothing about the emitted program's toolchain changes.
- **The compiler's build gains a step.** The runtime crate (or a `build.rs` step) must produce an archive for the linker's target. The compiler already builds Rust, so this is a new build artifact, not a new toolchain, but it is a real build-complexity cost (target/ABI matching, vendored deps if the runtime imports anything).
- **The FFI boundary is unchanged.** The IR calls C-ABI functions;whether the callee is C or Rust, the call shape is the same. Moving to Rust does not let the IR call Rust functions through a richer interface -- inkwell cannot express one. The ergonomic win is that the *runtime side* of each call is typed and single-sourced, not that IR construction gets easier. Hand-written IR construction on top of inkwell is untouched by any of this, and nothing in the well-established-Rust-library space sits above inkwell to replace it.



##The design round

The maintainer note frames one question ("should we use well-established Rust libraries rather than hand-writing everything"),but the survey splits it into two surfaces with genuinely different answers grounded above:

- **IR construction**:inkwell already is the well-established library;nothing well-established sits above it, and the hand-writing there is the 2808 lines of IR the backend fundamentally is. Not a replacement candidate
- **The runtime**:the C surface is where the documented friction lived,and a Rust runtime is a real, bounded change that does not touch the per-program toolchain.



Question for the round:

> Q41: **How should the native backend's runtime surface evolve: keep hand-written C, a Rust runtime crate, or a Rust runtime crate plus generated FFI declarations?**

> - **A**: status quo -- `runtime/toylang.c` keeps growing, three-way sync included
> - **B**: a thin Rust runtime crate (`extern "C"` surface, Rust internals),compiled into a static archive at compiler build time,linked by the existing `cc` call. Each helper's signature lives once;the internals type-check. Costs: a build artifact at compiler-build time, and the leak-everything memory posture must be re-decided in Rust terms
> - **C**: **B** plus a single table generating both the externs and the LLVM `add_function` declarations,so the three-way sync becomes a one-table update. Costs whatever B costs plus a small codegen mechanism
>
> A sub-question rides along only under B/C:whether the Rust source backend ([emit_rs.rs](emit_rs.rs)) may also link this crate (its self-contained one-`rustc`-call constraint at [lib.rs:370-396](lib.rs)) or stays standalone. Not part of the native-backend move itself;flag it for the round to rule on separately if it picks B/C