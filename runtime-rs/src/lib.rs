//! The native runtime in Rust. The C ABI (`tl_*` symbols, `tl_str` and `tl_vec` layouts) is fixed
//! by `src/emit_llvm.rs`; this crate reproduces it and replaces `runtime/toylang.c` one symbol
//! at a time. See plans/native-runtime-rust-research.md.

/// Proves the archive links into a compiled program and that `std` is usable inside it.
/// Nothing in the compiler's output calls this; `tests/native_runtime_link.rs` does.
#[unsafe(no_mangle)]
pub extern "C" fn tl_rt_smoke(n: i64) -> i64 {
    // Formatting goes through `alloc` and `core::fmt`, so a link that lacks a `std` dependency
    // fails here rather than in a later step.
    n.to_string().len() as i64
}
