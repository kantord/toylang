//! The native runtime in Rust. The C ABI (`tl_*` symbols, `tl_str` and `tl_vec` layouts) is fixed
//! by `src/emit_llvm.rs`; this crate reproduces it and replaces `runtime/toylang.c` one symbol
//! at a time. See plans/native-runtime-rust-research.md.
//!
//! A `tl_*` symbol is defined in exactly one of the two: each port deletes its C body in the
//! same commit that adds the Rust one.

use std::mem::{offset_of, size_of};

/// A toylang Str: bytes and a length, never null-terminated by contract. Field order is the C
/// `tl_str` in runtime/toylang.c, which still reads it; both sides assert the same offsets.
#[repr(C)]
pub struct TlStr {
    ptr: *const u8,
    len: i64,
}

const _: () = {
    assert!(size_of::<TlStr>() == 16);
    assert!(offset_of!(TlStr, ptr) == 0);
    assert!(offset_of!(TlStr, len) == 8);
};

/// The one place the runtime allocates a value. Nothing frees: the mutation model decides between
/// refcounting and tracing (runtime/toylang.c lines 7 to 10), and until it does every value is
/// leaked on purpose. A later change of memory policy is a change to this function.
fn leak_str(bytes: &[u8]) -> *mut TlStr {
    let ptr = Box::leak(bytes.to_vec().into_boxed_slice()).as_ptr();
    Box::into_raw(Box::new(TlStr {
        ptr,
        len: bytes.len() as i64,
    }))
}

/// JS's `String(number)`: shortest round-trip digits laid out by ECMA-262 Number::toString, so
/// NaN, `Infinity`, `-Infinity`, `-0` printing as `0`, fixed notation up to 21 digits and
/// scientific beyond are all ryu-js's rules, not restated here.
#[unsafe(no_mangle)]
pub extern "C" fn tl_float_to_str(x: f64) -> *mut TlStr {
    leak_str(ryu_js::Buffer::new().format(x).as_bytes())
}

/// Proves the archive links into a compiled program and that `std` is usable inside it.
/// Nothing in the compiler's output calls this; `tests/native_runtime_link.rs` does.
#[unsafe(no_mangle)]
pub extern "C" fn tl_rt_smoke(n: i64) -> i64 {
    // Formatting goes through `alloc` and `core::fmt`, so a link that lacks a `std` dependency
    // fails here rather than in a later step.
    n.to_string().len() as i64
}
