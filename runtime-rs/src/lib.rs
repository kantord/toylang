//! The native runtime in Rust. The C ABI (`tl_*` symbols, `tl_str` and `tl_vec` layouts) is fixed
//! by `src/emit_llvm.rs`; this crate reproduces it and replaces `runtime/toylang.c` one symbol
//! at a time. See plans/native-runtime-rust-research.md.
//!
//! A `tl_*` symbol is defined in exactly one of the two: each port deletes its C body in the
//! same commit that adds the Rust one.

// Every `extern "C"` function here is an entry point for generated code, never called from Rust,
// and takes pointers the compiler's IR guarantees valid. Marking them `unsafe fn` would change
// nothing about the ABI or the callers.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

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

/// The Str header. The one place a header is allocated, and the seam for `leak_str`.
///
/// Takes ownership of `bytes` (a block from C's `tl_alloc` or from `leak_str`) and copies
/// nothing: the C JSON parser builds its strings in a buffer and hands it over here. Nothing
/// frees. The mutation model decides between refcounting and tracing (runtime/toylang.c lines 7
/// to 10), and until it does every value is leaked on purpose.
#[unsafe(no_mangle)]
pub extern "C" fn tl_str_new(bytes: *const u8, len: i64) -> *mut TlStr {
    Box::into_raw(Box::new(TlStr { ptr: bytes, len }))
}

/// The one place the runtime allocates a value's bytes. A later change of memory policy is a
/// change to this function and `tl_str_new`.
fn leak_str(bytes: Vec<u8>) -> *mut TlStr {
    let len = bytes.len() as i64;
    tl_str_new(Box::leak(bytes.into_boxed_slice()).as_ptr(), len)
}

/// An empty buffer with room for `n` bytes. C's `tl_alloc` printed this and exited 1 when malloc
/// failed; Rust's allocation error handler would abort with SIGABRT, so this keeps the exit.
fn buffer(n: i64) -> Vec<u8> {
    let mut v = Vec::new();
    if usize::try_from(n).is_err() || v.try_reserve_exact(n as usize).is_err() {
        const MSG: &[u8] = b"toylang: out of memory\n";
        let _ = write_fd(2, MSG);
        std::process::exit(1);
    }
    v
}

/// The bytes of a Str. A zero-length Str may carry a null or dangling pointer (C's memcpy and
/// memcmp tolerated one), which `from_raw_parts` does not.
///
/// # Safety
/// `s` points at a live `TlStr` whose `ptr` is valid for `len` bytes.
unsafe fn bytes<'a>(s: *const TlStr) -> &'a [u8] {
    let s = unsafe { &*s };
    if s.len == 0 {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(s.ptr, s.len as usize) }
}

/// Writes straight to a descriptor: no buffer, so output interleaves in order with whatever C
/// writes to the same descriptor, and nothing needs flushing at exit (the generated `main`
/// bypasses Rust's `lang_start`, which is what flushes `std::io::stdout`). Retries partial
/// writes and EINTR, which the C `write` did not.
fn write_fd(fd: i32, data: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::fd::FromRawFd;
    let mut f = std::mem::ManuallyDrop::new(unsafe { std::fs::File::from_raw_fd(fd) });
    f.write_all(data)
}

#[unsafe(no_mangle)]
pub extern "C" fn tl_concat(a: *const TlStr, b: *const TlStr) -> *mut TlStr {
    let (a, b) = unsafe { (bytes(a), bytes(b)) };
    let mut out = buffer((a.len() + b.len()) as i64);
    out.extend_from_slice(a);
    out.extend_from_slice(b);
    leak_str(out)
}

#[unsafe(no_mangle)]
pub extern "C" fn tl_int_to_str(n: i64) -> *mut TlStr {
    leak_str(n.to_string().into_bytes())
}

#[unsafe(no_mangle)]
pub extern "C" fn tl_str_eq(a: *const TlStr, b: *const TlStr) -> i64 {
    (unsafe { bytes(a) == bytes(b) }) as i64
}

/// Byte order, which is what Lua does and what `memcmp` gave. JavaScript compares UTF-16 code
/// units, so the backends agree on ASCII and are not guaranteed to beyond it.
#[unsafe(no_mangle)]
pub extern "C" fn tl_str_cmp(a: *const TlStr, b: *const TlStr) -> i64 {
    unsafe { bytes(a).cmp(bytes(b)) as i64 }
}

/// A write error is ignored, as the C did: a closed stdout must not turn into a different
/// program result. One write for the payload and one for the newline, rather than copying to
/// join them.
#[unsafe(no_mangle)]
pub extern "C" fn tl_print(s: *const TlStr) {
    let _ = write_fd(1, unsafe { bytes(s) });
    let _ = write_fd(1, b"\n");
}

/// JSON string escaping, matching what the Lua and JavaScript printers emit, and NOT
/// `serde_json`: that writes `\b` and `\f` for 0x08 and 0x0c where every backend prints
/// `\u0008` and `\u000c`. Backslash and quote are escaped, `\n \r \t` have short forms, every
/// other byte below 0x20 is `\u00xx`, and DEL (0x7f) and all bytes from 0x80 up go out raw.
#[unsafe(no_mangle)]
pub extern "C" fn tl_quote(s: *const TlStr) -> *mut TlStr {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let s = unsafe { bytes(s) };
    let mut out = buffer(s.len() as i64 + 2);
    out.push(b'"');
    for &c in s {
        match c {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0..0x20 => out.extend_from_slice(&[
                b'\\',
                b'u',
                b'0',
                b'0',
                HEX[usize::from(c >> 4)],
                HEX[usize::from(c & 0xf)],
            ]),
            _ => out.push(c),
        }
    }
    out.push(b'"');
    leak_str(out)
}

/// The columnar Vec, opaque until it is ported: element access goes through the C accessor.
#[repr(C)]
pub struct TlVec {
    _opaque: [u8; 0],
}

unsafe extern "C" {
    fn tl_vec_len(v: *const TlVec) -> i64;
    fn tl_vec_get(v: *const TlVec, col: i64, i: i64) -> i64;
}

/// `open`, the parts (a Vec whose one column holds `TlStr` pointers) separated by `sep`, `close`.
/// One allocation, so printing a Vec is not quadratic in its length.
#[unsafe(no_mangle)]
pub extern "C" fn tl_str_join(
    parts: *const TlVec,
    open: *const TlStr,
    sep: *const TlStr,
    close: *const TlStr,
) -> *mut TlStr {
    let (open, sep, close) = unsafe { (bytes(open), bytes(sep), bytes(close)) };
    let n = unsafe { tl_vec_len(parts) };
    let parts: Vec<&[u8]> = (0..n)
        .map(|i| unsafe { bytes(tl_vec_get(parts, 0, i) as *const TlStr) })
        .collect();
    let seps = sep.len() * parts.len().saturating_sub(1);
    let total = open.len() + close.len() + seps + parts.iter().map(|p| p.len()).sum::<usize>();
    let mut out = buffer(total as i64);
    out.extend_from_slice(open);
    for (i, part) in parts.iter().enumerate() {
        if i > 0 {
            out.extend_from_slice(sep);
        }
        out.extend_from_slice(part);
    }
    out.extend_from_slice(close);
    leak_str(out)
}

/// JS's `String(number)`: shortest round-trip digits laid out by ECMA-262 Number::toString, so
/// NaN, `Infinity`, `-Infinity`, `-0` printing as `0`, fixed notation up to 21 digits and
/// scientific beyond are all ryu-js's rules, not restated here.
#[unsafe(no_mangle)]
pub extern "C" fn tl_float_to_str(x: f64) -> *mut TlStr {
    leak_str(ryu_js::Buffer::new().format(x).as_bytes().to_vec())
}

/// Proves the archive links into a compiled program and that `std` is usable inside it.
/// Nothing in the compiler's output calls this; `tests/native_runtime_link.rs` does.
#[unsafe(no_mangle)]
pub extern "C" fn tl_rt_smoke(n: i64) -> i64 {
    // Formatting goes through `alloc` and `core::fmt`, so a link that lacks a `std` dependency
    // fails here rather than in a later step.
    n.to_string().len() as i64
}
