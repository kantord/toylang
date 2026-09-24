/* The native backend's runtime.
 *
 * Compiled and linked into every native binary by the `cc` invocation that already links the
 * object file. The generated LLVM IR declares these and calls them; nothing here knows about
 * toylang's types beyond what its signatures say.
 *
 * Nothing frees. Prototype 1.5 leaks deliberately: choosing between refcounting and tracing
 * belongs with the mutation model, and a half-built refcount would be worse than an honest
 * leak in a program that runs once and exits. Keeping every allocation in this file keeps that
 * decision in one visible place.
 */

/* No behaviour is left here. The last group, `tl_pipe_through` and the allocation and buffer
 * helpers it used, moved to runtime-rs/src/pipe.rs; every other `tl_*` symbol was already
 * there. The file is still compiled and linked until the row that deletes it, together with
 * the `RUNTIME_C` constant in src/emit_llvm.rs, lands. */
