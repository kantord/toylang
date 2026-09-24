//! The native runtime in Rust. The C ABI (`tl_*` symbols, `tl_str` and `tl_vec` layouts) is fixed
//! by `src/emit_llvm.rs`; this crate reproduces it and replaces `runtime/toylang.c` one symbol
//! at a time. See plans/native-runtime-rust-research.md.
//!
//! A `tl_*` symbol is defined in exactly one of the two: each port deletes its C body in the
//! same commit that adds the Rust one.

use std::alloc::{Layout, alloc};
use std::cmp::Ordering;
use std::mem::{offset_of, size_of};
use std::ptr::{NonNull, null_mut};

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

/// C's `tl_alloc` printed this and exited 1 when malloc failed; Rust's allocation error handler
/// would abort with SIGABRT, so every allocation here that can fail on size keeps the exit.
fn out_of_memory() -> ! {
    const MSG: &[u8] = b"toylang: out of memory\n";
    let _ = write_fd(2, MSG);
    std::process::exit(1);
}

/// An empty buffer with room for `n` bytes.
fn buffer(n: i64) -> Vec<u8> {
    let mut v = Vec::new();
    if usize::try_from(n).is_err() || v.try_reserve_exact(n as usize).is_err() {
        out_of_memory();
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

/// # Safety
/// Each `*const TlStr` points at a live Str whose bytes are valid for its length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_concat(a: *const TlStr, b: *const TlStr) -> *mut TlStr {
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

/// # Safety
/// Each `*const TlStr` points at a live Str whose bytes are valid for its length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_str_eq(a: *const TlStr, b: *const TlStr) -> i64 {
    (unsafe { bytes(a) == bytes(b) }) as i64
}

/// Byte order, which is what Lua does and what `memcmp` gave. JavaScript compares UTF-16 code
/// units, so the backends agree on ASCII and are not guaranteed to beyond it.
///
/// # Safety
/// Each `*const TlStr` points at a live Str whose bytes are valid for its length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_str_cmp(a: *const TlStr, b: *const TlStr) -> i64 {
    unsafe { bytes(a).cmp(bytes(b)) as i64 }
}

/// A write error is ignored, as the C did: a closed stdout must not turn into a different
/// program result. One write for the payload and one for the newline, rather than copying to
/// join them.
///
/// # Safety
/// Each `*const TlStr` points at a live Str whose bytes are valid for its length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_print(s: *const TlStr) {
    let _ = write_fd(1, unsafe { bytes(s) });
    let _ = write_fd(1, b"\n");
}

/// JSON string escaping, matching what the Lua and JavaScript printers emit, and NOT
/// `serde_json`: that writes `\b` and `\f` for 0x08 and 0x0c where every backend prints
/// `\u0008` and `\u000c`. Backslash and quote are escaped, `\n \r \t` have short forms, every
/// other byte below 0x20 is `\u00xx`, and DEL (0x7f) and all bytes from 0x80 up go out raw.
///
/// # Safety
/// Each `*const TlStr` points at a live Str whose bytes are valid for its length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_quote(s: *const TlStr) -> *mut TlStr {
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

/// A Vec: a length and `ncols` columns, each holding `len` raw 8-byte slots.
///
/// The layout is struct of arrays. A Vec of scalars has one column; a Vec of records has one
/// column per field, which is what makes reading a field off a Vec a column rather than a
/// gather. Every scalar toylang has fits one slot (an Int is an i64; a Str, a record or a nested
/// Vec is a pointer stored as an i64), so one set of functions serves every element type.
///
/// A column of a zero-length Vec is null, and `cols` of a Vec with no columns is dangling:
/// nothing may read through either, and `slice_of` is what turns them into an empty slice.
/// The field order is the C `tl_vec` in runtime/toylang.c, which still reads it directly; both
/// sides assert the same offsets. The IR from src/emit_llvm.rs reaches a Vec only through the
/// accessors below.
#[repr(C)]
pub struct TlVec {
    len: i64,
    ncols: i64,
    cols: *mut *mut i64,
}

const _: () = {
    assert!(size_of::<TlVec>() == 24);
    assert!(offset_of!(TlVec, len) == 0);
    assert!(offset_of!(TlVec, ncols) == 8);
    assert!(offset_of!(TlVec, cols) == 16);
};

/// The one place the runtime allocates anything but a Str's bytes (see `leak_str`): Vec headers,
/// columns, masks, records and Opt boxes all come from here, so a later change of memory policy
/// is this function plus `leak_str`. Nothing frees them.
///
/// The block is uninitialized, as malloc's was; every caller fills what it reads. `n` is a
/// count of `T`, and a negative or overflowing one is the out-of-memory exit that `(size_t)n * 8`
/// reaching malloc was in C. A zero count gets a dangling pointer, as glibc's `malloc(0)` gave a
/// non-null one: a zero-field record must not look like an absent Opt.
fn leak_array<T>(n: i64) -> *mut T {
    let Some(layout) = usize::try_from(n)
        .ok()
        .and_then(|n| Layout::array::<T>(n).ok())
    else {
        out_of_memory()
    };
    if layout.size() == 0 {
        return NonNull::dangling().as_ptr();
    }
    let p = unsafe { alloc(layout) };
    if p.is_null() {
        out_of_memory();
    }
    p.cast()
}

/// The one place a Vec header is allocated.
fn leak_vec(len: i64, ncols: i64, cols: *mut *mut i64) -> *mut TlVec {
    let v = leak_array::<TlVec>(1);
    unsafe { v.write(TlVec { len, ncols, cols }) };
    v
}

/// `len` elements at `p`, or an empty slice when `len` is not positive: `p` is null for a
/// zero-length column or mask, and `from_raw_parts` does not accept that.
///
/// # Safety
/// When `len` is positive, `p` is valid for `len` reads of `T` and nothing writes them for `'a`.
unsafe fn slice_of<'a, T>(p: *const T, len: i64) -> &'a [T] {
    if len <= 0 {
        return &[];
    }
    unsafe { std::slice::from_raw_parts(p, len as usize) }
}

/// # Safety
/// As `slice_of`, for writes, and nothing else reads or writes the elements for `'a`.
unsafe fn slice_of_mut<'a, T>(p: *mut T, len: i64) -> &'a mut [T] {
    if len <= 0 {
        return &mut [];
    }
    unsafe { std::slice::from_raw_parts_mut(p, len as usize) }
}

/// Column `col` of `v`, as the pointer stored in `cols`.
///
/// # Safety
/// `v` points at a live `TlVec` and `col` is in `0..ncols`.
unsafe fn column_ptr(v: *const TlVec, col: i64) -> *mut i64 {
    let v = unsafe { &*v };
    debug_assert!((0..v.ncols).contains(&col), "column {col} of {}", v.ncols);
    unsafe { *v.cols.add(col as usize) }
}

/// Column `col` of `v` as a slice of its `len` slots.
///
/// # Safety
/// As `column_ptr`, and the column holds `len` slots that nothing writes for `'a`.
unsafe fn column<'a>(v: *const TlVec, col: i64) -> &'a [i64] {
    let len = unsafe { (*v).len };
    // Not even `cols[col]` is read for an empty Vec: the C never touched it, and a Vec with no
    // columns has a dangling `cols`.
    if len <= 0 {
        return &[];
    }
    unsafe { slice_of(column_ptr(v, col), len) }
}

/// Column `col` of `v` as a mutable slice of its `len` slots.
///
/// # Safety
/// As `column`, and nothing else reads or writes the column for `'a`.
unsafe fn column_mut<'a>(v: *mut TlVec, col: i64) -> &'a mut [i64] {
    let len = unsafe { (*v).len };
    if len <= 0 {
        return &mut [];
    }
    unsafe { slice_of_mut(column_ptr(v, col), len) }
}

/// `len` rows of `ncols` columns, every slot uninitialized: the caller fills them.
#[unsafe(no_mangle)]
pub extern "C" fn tl_vec_new(len: i64, ncols: i64) -> *mut TlVec {
    let cols = leak_array::<*mut i64>(ncols);
    for c in 0..ncols {
        let col = if len > 0 {
            leak_array::<i64>(len)
        } else {
            null_mut()
        };
        unsafe { cols.add(c as usize).write(col) };
    }
    leak_vec(len, ncols, cols)
}

/// # Safety
/// `v` points at a live `TlVec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_len(v: *const TlVec) -> i64 {
    unsafe { (*v).len }
}

/// No bounds check, as in C: the IR the compiler emits checks the index before it gets here.
///
/// # Safety
/// `v` points at a live `TlVec`, `col` is in `0..ncols` and `i` is in `0..len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_get(v: *const TlVec, col: i64, i: i64) -> i64 {
    debug_assert!(
        (0..unsafe { (*v).len }).contains(&i),
        "index {i} of {}",
        unsafe { (*v).len }
    );
    unsafe { *column_ptr(v, col).add(i as usize) }
}

/// # Safety
/// As `tl_vec_get`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_set(v: *mut TlVec, col: i64, i: i64, value: i64) {
    debug_assert!(
        (0..unsafe { (*v).len }).contains(&i),
        "index {i} of {}",
        unsafe { (*v).len }
    );
    unsafe { *column_ptr(v, col).add(i as usize) = value }
}

/// One field of a Vec of records, as a Vec of that field's type. The column is shared rather
/// than copied, so `.name` on a Vec<User> costs one small header and no element work; this is
/// the whole reason for the struct-of-arrays layout.
///
/// # Safety
/// `v` points at a live `TlVec` and `col` is in `0..ncols`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_column(v: *const TlVec, col: i64) -> *mut TlVec {
    let cols = leak_array::<*mut i64>(1);
    unsafe { cols.write(column_ptr(v, col)) };
    leak_vec(unsafe { (*v).len }, 1, cols)
}

/// `select`: the rows of `src` whose mask byte is nonzero, every column compacted with the same
/// surviving indices. Counting first and then filling beats growing an array.
///
/// # Safety
/// `src` points at a live `TlVec` and `keep` at `src.len` mask bytes (null when `len` is zero).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_from_mask(src: *const TlVec, keep: *const i8) -> *mut TlVec {
    let (ncols, keep) = unsafe { ((*src).ncols, slice_of(keep, (*src).len)) };
    let n = keep.iter().filter(|&&k| k != 0).count() as i64;
    let out = tl_vec_new(n, ncols);
    for c in 0..ncols {
        let survivors = unsafe { column(src, c) }
            .iter()
            .zip(keep)
            .filter_map(|(slot, &k)| (k != 0).then_some(slot));
        for (dst, &slot) in unsafe { slice_of_mut(column_ptr(out, c), n) }
            .iter_mut()
            .zip(survivors)
        {
            *dst = slot;
        }
    }
    out
}

/// Null for a length of zero, as in C.
#[unsafe(no_mangle)]
pub extern "C" fn tl_mask_new(len: i64) -> *mut i8 {
    if len > 0 {
        leak_array::<i8>(len)
    } else {
        null_mut()
    }
}

/// # Safety
/// `mask` is a block from `tl_mask_new` and `i` is within its length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_mask_set(mask: *mut i8, i: i64, value: i64) {
    unsafe { *mask.add(i as usize) = (value != 0) as i8 }
}

/// A record: one slot per field, in the field order the type declares. Records only ever arrive
/// from input, since the language has no expression that builds one.
#[unsafe(no_mangle)]
pub extern "C" fn tl_rec_new(nfields: i64) -> *mut i64 {
    leak_array(nfields)
}

/// # Safety
/// `r` is a record with more than `field` fields.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_rec_get(r: *const i64, field: i64) -> i64 {
    unsafe { *r.add(field as usize) }
}

/// # Safety
/// As `tl_rec_get`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_rec_set(r: *mut i64, field: i64, value: i64) {
    unsafe { *r.add(field as usize) = value }
}

/// Opt: a pointer to a slot, or null for absent. Boxing rather than a tag pair, because a slot
/// holds any value an Int can and there is no spare bit pattern to mean absent. Uniform across
/// element types, which is what lets one function serve them all.
#[unsafe(no_mangle)]
pub extern "C" fn tl_opt_some(value: i64) -> *mut i64 {
    let p = leak_array::<i64>(1);
    unsafe { p.write(value) };
    p
}

#[unsafe(no_mangle)]
pub extern "C" fn tl_opt_is_some(o: *const i64) -> i64 {
    !o.is_null() as i64
}

/// # Safety
/// `o` is a non-null Opt from `tl_opt_some`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_opt_get(o: *const i64) -> i64 {
    unsafe { *o }
}

/// `open`, the parts (a Vec whose one column holds `TlStr` pointers) separated by `sep`, `close`.
/// One allocation, so printing a Vec is not quadratic in its length.
///
/// # Safety
/// Each `TlStr` pointer is as for `tl_concat`, and `parts` is a Vec of `TlStr` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_str_join(
    parts: *const TlVec,
    open: *const TlStr,
    sep: *const TlStr,
    close: *const TlStr,
) -> *mut TlStr {
    let (open, sep, close) = unsafe { (bytes(open), bytes(sep), bytes(close)) };
    let parts: Vec<&[u8]> = unsafe { column(parts, 0) }
        .iter()
        .map(|&s| unsafe { bytes(s as *const TlStr) })
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

/// Gather row `i` of a Vec of records back into a record. The struct-of-arrays layout spreads an
/// element across columns and almost nothing needs it whole (`select` reads single fields, `.field`
/// returns a column), so this exists for what has to hand a whole element out: printing, indexing,
/// `first`, `max_by`.
///
/// # Safety
/// `v` points at a live `TlVec` and `i` is in `0..len`.
unsafe fn rec_from_vec(v: *const TlVec, i: i64) -> *mut i64 {
    let ncols = unsafe { (*v).ncols };
    let rec = tl_rec_new(ncols);
    for c in 0..ncols {
        unsafe { tl_rec_set(rec, c, tl_vec_get(v, c, i)) };
    }
    rec
}

/// An Opt holding row `i` of `v`: the record gathered out of the columns when `is_record`, else
/// the one slot. Column count cannot stand in for `is_record`, since a record with one field has
/// one column exactly as a Vec of scalars does.
///
/// # Safety
/// As `rec_from_vec`.
unsafe fn opt_of_row(v: *const TlVec, i: i64, is_record: i32) -> *mut i64 {
    let entry = if is_record != 0 {
        unsafe { rec_from_vec(v, i) as i64 }
    } else {
        unsafe { tl_vec_get(v, 0, i) }
    };
    tl_opt_some(entry)
}

/// Ascending order of two key slots: the raw value for Int, Int64 and Char (all three live in the
/// slot unnarrowed, and the checker already keeps a Char from mixing with the others), the bytes
/// for Str, whose slot is a `TlStr` pointer.
///
/// # Safety
/// When `is_str`, both slots are pointers to live Strs.
unsafe fn cmp_keys(is_str: bool, a: i64, b: i64) -> Ordering {
    if is_str {
        unsafe { bytes(a as *const TlStr).cmp(bytes(b as *const TlStr)) }
    } else {
        a.cmp(&b)
    }
}

/// `sort` over a Vec of Int, Int64 or Char: a sorted copy of the one column.
///
/// # Safety
/// `v` points at a live one-column `TlVec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_sort_int(v: *const TlVec) -> *mut TlVec {
    let out = tl_vec_new(unsafe { (*v).len }, 1);
    let col = unsafe { column_mut(out, 0) };
    col.copy_from_slice(unsafe { column(v, 0) });
    col.sort_unstable();
    out
}

/// `sort` over a Vec of Str, by bytes.
///
/// # Safety
/// `v` points at a live one-column `TlVec` of `TlStr` pointers.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_sort_str(v: *const TlVec) -> *mut TlVec {
    let out = tl_vec_new(unsafe { (*v).len }, 1);
    let col = unsafe { column_mut(out, 0) };
    col.copy_from_slice(unsafe { column(v, 0) });
    col.sort_by(|&a, &b| unsafe { cmp_keys(true, a, b) });
    out
}

/// `sort_by` over a Vec of any element type. `keys` is the one-column Vec of projected keys, row
/// for row with `v`, and `is_str` says whether a key slot is a `TlStr` pointer. Sorting an index
/// vector with a stable sort keeps equal keys in their original order, and every column of `v` is
/// then permuted by it, as `tl_vec_reverse` does.
///
/// # Safety
/// `v` points at a live `TlVec`, `keys` at a one-column `TlVec` of the same length, and when
/// `is_str` the key slots are pointers to live Strs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_sort_by(
    v: *const TlVec,
    keys: *const TlVec,
    is_str: i32,
) -> *mut TlVec {
    let ncols = unsafe { (*v).ncols };
    let out = tl_vec_new(unsafe { (*v).len }, ncols);
    let keys = unsafe { column(keys, 0) };
    let mut order: Vec<usize> = (0..keys.len()).collect();
    if is_str != 0 {
        order.sort_by_key(|&i| unsafe { bytes(keys[i] as *const TlStr) });
    } else {
        order.sort_by_key(|&i| keys[i]);
    }
    for c in 0..ncols {
        let src = unsafe { column(v, c) };
        for (dst, &i) in unsafe { column_mut(out, c) }.iter_mut().zip(&order) {
            *dst = src[i];
        }
    }
    out
}

/// `max_by`: the entry with the greatest key, the first of equal maxima. A hand loop with a
/// strict greater-than, because `Iterator::max_by_key` returns the LAST of equal maxima. Null on
/// an empty Vec, the absence encoding `tl_opt_some` uses everywhere else.
///
/// # Safety
/// As `tl_vec_sort_by`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_max_by(
    v: *const TlVec,
    keys: *const TlVec,
    is_str: i32,
    is_record: i32,
) -> *mut i64 {
    if unsafe { (*v).len } == 0 {
        return null_mut();
    }
    let keys = unsafe { column(keys, 0) };
    let mut best = 0;
    for (i, &key) in keys.iter().enumerate().skip(1) {
        if unsafe { cmp_keys(is_str != 0, key, keys[best]) } == Ordering::Greater {
            best = i;
        }
    }
    unsafe { opt_of_row(v, best as i64, is_record) }
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

/// Run with `cargo test -p toylang-rt`; the workspace's default test run does not include this
/// crate. The debug build keeps the `debug_assert!` bounds checks in the accessors, which the
/// `runtime` profile compiles out, so an out-of-range access in the Vec core aborts the test
/// binary here (a panic in an `extern "C"` function cannot unwind).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_and_columns_round_trip() {
        unsafe {
            let v = tl_vec_new(3, 2);
            for i in 0..3 {
                tl_vec_set(v, 0, i, i * 10);
                tl_vec_set(v, 1, i, -i);
            }
            assert_eq!(tl_vec_len(v), 3);
            assert_eq!(tl_vec_get(v, 0, 2), 20);
            assert_eq!(tl_vec_get(v, 1, 2), -2);

            let col = tl_vec_column(v, 1);
            assert_eq!((tl_vec_len(col), tl_vec_get(col, 0, 1)), (3, -1));
            tl_vec_set(col, 0, 0, 99);
            assert_eq!(tl_vec_get(v, 1, 0), 99, "a column is shared, not copied");
        }
    }

    #[test]
    fn empty_vecs_and_masks() {
        unsafe {
            let empty = tl_vec_new(0, 3);
            assert_eq!(tl_vec_len(empty), 0);
            assert!(column_ptr(empty, 2).is_null());
            let none = tl_vec_new(4, 0);
            assert_eq!((tl_vec_len(none), (*none).ncols), (4, 0));

            let kept = tl_vec_from_mask(empty, tl_mask_new(0));
            assert_eq!((tl_vec_len(kept), (*kept).ncols), (0, 3));
            assert!(tl_mask_new(0).is_null());
            assert!(
                !tl_rec_new(0).is_null(),
                "a zero-field record is not an absent Opt"
            );
        }
    }

    #[test]
    fn from_mask_compacts_every_column_together() {
        unsafe {
            let v = tl_vec_new(4, 2);
            for i in 0..4 {
                tl_vec_set(v, 0, i, i);
                tl_vec_set(v, 1, i, 100 + i);
            }
            let mask = tl_mask_new(4);
            for (i, keep) in [7, 0, 1, 0].into_iter().enumerate() {
                tl_mask_set(mask, i as i64, keep);
            }
            let out = tl_vec_from_mask(v, mask);
            assert_eq!(tl_vec_len(out), 2);
            assert_eq!((tl_vec_get(out, 0, 0), tl_vec_get(out, 1, 0)), (0, 100));
            assert_eq!((tl_vec_get(out, 0, 1), tl_vec_get(out, 1, 1)), (2, 102));
        }
    }

    #[test]
    fn records_and_opts() {
        unsafe {
            let r = tl_rec_new(2);
            tl_rec_set(r, 1, 5);
            tl_rec_set(r, 0, -1);
            assert_eq!((tl_rec_get(r, 0), tl_rec_get(r, 1)), (-1, 5));
            let some = tl_opt_some(0);
            assert_eq!((tl_opt_is_some(some), tl_opt_get(some)), (1, 0));
            assert_eq!(tl_opt_is_some(null_mut()), 0);
        }
    }

    /// A Vec with one column per slice, every column the same length.
    unsafe fn vec_of(cols: &[&[i64]]) -> *mut TlVec {
        let v = tl_vec_new(
            cols.first().map_or(0, |c| c.len() as i64),
            cols.len() as i64,
        );
        for (c, col) in cols.iter().enumerate() {
            for (i, &x) in col.iter().enumerate() {
                unsafe { tl_vec_set(v, c as i64, i as i64, x) };
            }
        }
        v
    }

    unsafe fn str_slot(s: &str) -> i64 {
        leak_str(s.as_bytes().to_vec()) as i64
    }

    unsafe fn column_of(v: *const TlVec, col: i64) -> Vec<i64> {
        unsafe { column(v, col) }.to_vec()
    }

    #[test]
    fn sort_int_orders_by_value_without_touching_the_input() {
        unsafe {
            let v = vec_of(&[&[3, i64::MIN, 7, i64::MAX, -1, 7]]);
            let out = tl_vec_sort_int(v);
            assert_eq!(
                column_of(out, 0),
                [i64::MIN, -1, 3, 7, 7, i64::MAX],
                "a subtraction comparator would overflow on the extremes"
            );
            assert_eq!(column_of(v, 0), [3, i64::MIN, 7, i64::MAX, -1, 7]);
            assert_eq!(tl_vec_len(tl_vec_sort_int(tl_vec_new(0, 1))), 0);
        }
    }

    #[test]
    fn strs_sort_by_bytes_not_by_locale_or_length() {
        unsafe {
            let words = ["b", "a", "B", "\u{e9}", "z", "", "ab"];
            let slots: Vec<i64> = words.iter().map(|w| str_slot(w)).collect();
            let out = tl_vec_sort_str(vec_of(&[&slots]));
            let sorted: Vec<&[u8]> = column_of(out, 0)
                .iter()
                .map(|&s| bytes(s as *const TlStr))
                .collect();
            let want: [&[u8]; 7] = [b"", b"B", b"a", b"ab", b"b", b"z", "\u{e9}".as_bytes()];
            assert_eq!(sorted, want);
        }
    }

    #[test]
    fn sort_by_is_stable_and_permutes_every_column() {
        unsafe {
            // Keys 2 1 2 1 2: the rows tagged 10 12 14 and 11 13 must keep their order.
            let v = vec_of(&[&[10, 11, 12, 13, 14], &[100, 101, 102, 103, 104]]);
            let keys = vec_of(&[&[2, 1, 2, 1, 2]]);
            let out = tl_vec_sort_by(v, keys, 0);
            assert_eq!(column_of(out, 0), [11, 13, 10, 12, 14]);
            assert_eq!(column_of(out, 1), [101, 103, 100, 102, 104]);
            assert_eq!(column_of(v, 0), [10, 11, 12, 13, 14], "input is untouched");

            let (x, y) = (str_slot("x"), str_slot("y"));
            let str_keys = vec_of(&[&[y, x, y, x, y]]);
            let out = tl_vec_sort_by(v, str_keys, 1);
            assert_eq!(column_of(out, 0), [11, 13, 10, 12, 14]);

            let empty = tl_vec_sort_by(tl_vec_new(0, 2), tl_vec_new(0, 1), 0);
            assert_eq!((tl_vec_len(empty), (*empty).ncols), (0, 2));
        }
    }

    #[test]
    fn max_by_takes_the_first_of_equal_maxima() {
        unsafe {
            let v = vec_of(&[&[10, 11, 12, 13], &[20, 21, 22, 23]]);
            let keys = vec_of(&[&[1, 5, 5, 2]]);
            assert_eq!(tl_opt_get(tl_vec_max_by(v, keys, 0, 0)), 11);

            let rec = tl_vec_max_by(v, keys, 0, 1);
            let rec = tl_opt_get(rec) as *const i64;
            assert_eq!((tl_rec_get(rec, 0), tl_rec_get(rec, 1)), (11, 21));

            let (a, b) = (str_slot("b"), str_slot("a"));
            let same = vec_of(&[&[a, b, a]]);
            let out = tl_vec_max_by(same, same, 1, 0);
            assert_eq!(tl_opt_get(out), a, "the first \"b\", not the last");

            assert!(tl_vec_max_by(tl_vec_new(0, 1), tl_vec_new(0, 1), 0, 0).is_null());
        }
    }
}
