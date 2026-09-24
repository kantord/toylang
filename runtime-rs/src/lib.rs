//! The native runtime in Rust. The C ABI (`tl_*` symbols, `tl_str` and `tl_vec` layouts) is fixed
//! by `src/emit_llvm.rs`. It replaced a single C file that `cc` compiled alongside every program;
//! see plans/native-runtime-rust-research.md.

mod input;
mod json;
mod pipe;

use std::alloc::{Layout, alloc};
use std::cmp::Ordering;
use std::mem::{offset_of, size_of};
use std::ptr::{NonNull, null_mut};

/// A toylang Str: bytes and a length, never null-terminated by contract. The IR from
/// src/emit_llvm.rs builds and reads this layout.
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
/// Takes ownership of `bytes` and copies nothing. Nothing frees. The mutation
/// model decides between refcounting and tracing, and a half-built refcount would be worse than
/// an honest leak in a program that runs once and exits, so until it does every value is leaked
/// on purpose.
fn str_new(bytes: *const u8, len: i64) -> *mut TlStr {
    Box::into_raw(Box::new(TlStr { ptr: bytes, len }))
}

/// The one place the runtime allocates a value's bytes. A later change of memory policy is a
/// change to this function and `str_new`.
fn leak_str(bytes: Vec<u8>) -> *mut TlStr {
    let len = bytes.len() as i64;
    str_new(Box::leak(bytes.into_boxed_slice()).as_ptr(), len)
}

/// A runtime failure: the message on stderr and exit 1, the way every refusal the checker could
/// not see ahead of time ends (`tl_div_by_zero` is the same).
fn fail(msg: &str) -> ! {
    let _ = write_fd(2, format!("toylang: {msg}\n").as_bytes());
    std::process::exit(1);
}

/// A refusal of the program's input: `toylang: input: <what> at <path>`, exit 1. An empty `path`
/// reads as `input`, the root.
fn fail_at(what: &str, path: &str) -> ! {
    let path = if path.is_empty() { "input" } else { path };
    fail(&format!("input: {what} at {path}"))
}

/// The only way arithmetic can fail.
#[unsafe(no_mangle)]
pub extern "C" fn tl_div_by_zero() -> ! {
    fail("divided by zero")
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

/// Writes straight to a descriptor: no buffer, so output interleaves in order with whatever else
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
/// The IR from src/emit_llvm.rs reaches a Vec only through the accessors below.
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

/// A one-column Vec holding `slots`.
fn vec_of_slots(slots: &[i64]) -> *mut TlVec {
    let v = tl_vec_new(slots.len() as i64, 1);
    unsafe { column_mut(v, 0) }.copy_from_slice(slots);
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

/// No bounds check: the IR the compiler emits checks the index before it gets here.
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

/// Null for a length of zero.
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
/// `first`, `max_by`. The only gather.
///
/// # Safety
/// `v` points at a live `TlVec` and `i` is in `0..len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_rec_from_vec(v: *const TlVec, i: i64) -> *mut i64 {
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
/// As `tl_rec_from_vec`.
unsafe fn opt_of_row(v: *const TlVec, i: i64, is_record: i32) -> *mut i64 {
    let entry = if is_record != 0 {
        unsafe { tl_rec_from_vec(v, i) as i64 }
    } else {
        unsafe { tl_vec_get(v, 0, i) }
    };
    tl_opt_some(entry)
}

/// Every element but the first, as an Opt: null on an empty Vec, the absence encoding
/// `tl_opt_some` uses everywhere else.
///
/// # Safety
/// `v` points at a live `TlVec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_tail(v: *const TlVec) -> *mut i64 {
    let (len, ncols) = unsafe { ((*v).len, (*v).ncols) };
    if len == 0 {
        return null_mut();
    }
    let out = tl_vec_new(len - 1, ncols);
    for c in 0..ncols {
        unsafe { column_mut(out, c).copy_from_slice(&column(v, c)[1..]) };
    }
    tl_opt_some(out as i64)
}

/// `first` over a Vec of any element type: an Opt of the first entry, null when empty.
/// `is_record` decides whether the entry has to be gathered out of the columns.
///
/// # Safety
/// `v` points at a live `TlVec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_first(v: *const TlVec, is_record: i32) -> *mut i64 {
    if unsafe { (*v).len } == 0 {
        return null_mut();
    }
    unsafe { opt_of_row(v, 0, is_record) }
}

/// `any` over a Vec<Bool>, which widens to a 0/1 slot: whether any slot is nonzero. False when
/// empty.
///
/// # Safety
/// `v` points at a live one-column `TlVec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_any(v: *const TlVec) -> i64 {
    unsafe { column(v, 0) }.iter().any(|&b| b != 0) as i64
}

/// `all` over a Vec<Bool>: whether every slot is nonzero, vacuously true when empty.
///
/// # Safety
/// As `tl_vec_any`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_all(v: *const TlVec) -> i64 {
    unsafe { column(v, 0) }.iter().all(|&b| b != 0) as i64
}

/// The inner Vecs of a Vec<Vec<T>>: its one column holds `TlVec` pointers.
///
/// # Safety
/// `vv` points at a live one-column `TlVec` of pointers to live `TlVec`s.
unsafe fn inner_vecs<'a>(vv: *const TlVec) -> impl Iterator<Item = *const TlVec> + 'a {
    unsafe { column(vv, 0) }
        .iter()
        .map(|&slot| slot as *const TlVec)
}

/// Flatten a Vec<Vec<T>> into a Vec<T>. `ncols` is T's column count, passed in rather than read
/// off an inner Vec because an empty outer Vec has no inner Vec to read it from.
///
/// # Safety
/// As `inner_vecs`, and every inner Vec has `ncols` columns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_flatten(vv: *const TlVec, ncols: i64) -> *mut TlVec {
    let total =
        unsafe { inner_vecs(vv) }.fold(0i64, |n, inner| n.saturating_add(unsafe { (*inner).len }));
    let out = tl_vec_new(total, ncols);
    let mut at = 0;
    for inner in unsafe { inner_vecs(vv) } {
        let len = unsafe { (*inner).len } as usize;
        for c in 0..ncols {
            unsafe { column_mut(out, c)[at..at + len].copy_from_slice(column(inner, c)) };
        }
        at += len;
    }
    out
}

/// `a + b` on two Vecs (kantord/toylang#97): join them without the one level of nesting
/// `tl_vec_flatten` unwraps. `ncols` is T's column count, for the same reason.
///
/// # Safety
/// `a` and `b` point at live `TlVec`s with `ncols` columns each.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_concat(a: *const TlVec, b: *const TlVec, ncols: i64) -> *mut TlVec {
    let (alen, blen) = unsafe { ((*a).len, (*b).len) };
    let out = tl_vec_new(alen.saturating_add(blen), ncols);
    for c in 0..ncols {
        let (head, tail) = unsafe { column_mut(out, c) }.split_at_mut(alen as usize);
        head.copy_from_slice(unsafe { column(a, c) });
        tail.copy_from_slice(unsafe { column(b, c) });
    }
    out
}

/// `transpose` of a rectangular `Vec<Vec<T>>`: row `i` of the result is column `i` of the input.
/// `ncols` is T's column count, as for `tl_vec_flatten`. A ragged input is refused at runtime,
/// since the checker cannot see lengths; the result of an empty input is the empty
/// `Vec<Vec<T>>`.
///
/// # Safety
/// As `tl_vec_flatten`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_transpose(vv: *const TlVec, ncols: i64) -> *mut TlVec {
    let Some(first) = (unsafe { inner_vecs(vv) }).next() else {
        return tl_vec_new(0, 1);
    };
    let width = unsafe { (*first).len };
    if unsafe { inner_vecs(vv) }.any(|inner| unsafe { (*inner).len } != width) {
        fail("transpose needs a rectangular Vec of Vecs");
    }
    let nrows = unsafe { (*vv).len };
    let out = tl_vec_new(width, 1);
    for (c, slot) in unsafe { column_mut(out, 0) }.iter_mut().enumerate() {
        let row = tl_vec_new(nrows, ncols);
        for k in 0..ncols {
            let dst = unsafe { column_mut(row, k) };
            for (d, inner) in dst.iter_mut().zip(unsafe { inner_vecs(vv) }) {
                *d = unsafe { column(inner, k) }[c];
            }
        }
        *slot = row as i64;
    }
    out
}

/// `reverse`, generic over the element type like `tl_vec_tail`: every column's row order flips
/// together, so a Vec of records or of nested Vecs reverses with no type-specific code.
///
/// # Safety
/// `v` points at a live `TlVec` with `ncols` columns.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_reverse(v: *const TlVec, ncols: i64) -> *mut TlVec {
    let out = tl_vec_new(unsafe { (*v).len }, ncols);
    for c in 0..ncols {
        let src = unsafe { column(v, c) }.iter().rev();
        for (dst, &slot) in unsafe { column_mut(out, c) }.iter_mut().zip(src) {
            *dst = slot;
        }
    }
    out
}

/// `sum` over a Vec of Int or Int64 (kantord/toylang#140): the reduction of `+`, each addition
/// wrapping the way the language's `+` wraps. Both widths live in the same i64 slot, so `narrow`
/// is what tells Int (wrap to 32 bits, sign-extended, after every addition) from Int64 (no
/// narrowing).
///
/// # Safety
/// `v` points at a live one-column `TlVec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_sum(v: *const TlVec, narrow: i32) -> i64 {
    unsafe { column(v, 0) }.iter().fold(0i64, |acc, &x| {
        let acc = acc.wrapping_add(x);
        if narrow != 0 { acc as i32 as i64 } else { acc }
    })
}

/// `max` over a Vec of Int or Int64, as an Opt: null when empty. Int's slots are already
/// sign-extended, so the i64 comparison orders them correctly.
///
/// # Safety
/// `v` points at a live one-column `TlVec`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_max(v: *const TlVec) -> *mut i64 {
    match unsafe { column(v, 0) }.iter().max() {
        Some(&m) => tl_opt_some(m),
        None => null_mut(),
    }
}

/// Index `i` of `v`, `depth` layers down, counting from the end when negative. At depth zero
/// the result is an Opt (null when out of range); below it, a Vec of the results for each inner
/// Vec. `is_record` decides whether an entry is gathered out of the columns.
///
/// # Safety
/// `v` points at a live `TlVec` (one column of `TlVec` pointers for every layer above depth 0).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_at(v: *const TlVec, i: i64, depth: i64, is_record: i32) -> *mut i64 {
    if depth > 0 {
        let out = tl_vec_new(unsafe { (*v).len }, 1);
        let inner = unsafe { column(v, 0) };
        for (dst, &slot) in unsafe { column_mut(out, 0) }.iter_mut().zip(inner) {
            *dst = unsafe { tl_at(slot as *const TlVec, i, depth - 1, is_record) } as i64;
        }
        return out.cast();
    }
    let len = unsafe { (*v).len };
    // `len` is not negative, so this cannot overflow.
    let i = if i < 0 { len + i } else { i };
    if !(0..len).contains(&i) {
        return null_mut();
    }
    unsafe { opt_of_row(v, i, is_record) }
}

/// Lazy `select`: read through the survivor mask the codegen built inline, without compacting
/// the source first. The predicate never runs here; the mask is what keeps the codegen's own
/// mask-building loop vectorisable.
///
/// # Safety
/// `src` points at a live `TlVec` and `keep` at `src.len` mask bytes (null when `len` is zero).
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_sel_len(src: *const TlVec, keep: *const i8) -> i64 {
    unsafe { slice_of(keep, (*src).len) }
        .iter()
        .filter(|&&k| k != 0)
        .count() as i64
}

/// The i-th survivor of `tl_sel_len`'s mask, as `tl_at` finds the i-th element of a dense Vec:
/// negative counts from the end, null when out of range.
///
/// # Safety
/// As `tl_sel_len`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_sel_at(
    src: *const TlVec,
    keep: *const i8,
    i: i64,
    is_record: i32,
) -> *mut i64 {
    let keep = unsafe { slice_of(keep, (*src).len) };
    let survivors = keep.iter().filter(|&&k| k != 0).count() as i64;
    let i = if i < 0 { survivors + i } else { i };
    if !(0..survivors).contains(&i) {
        return null_mut();
    }
    let row = keep
        .iter()
        .enumerate()
        .filter(|&(_, &k)| k != 0)
        .nth(i as usize)
        .map(|(row, _)| row);
    // `i` is below the survivor count, so `nth` found a row.
    unsafe { opt_of_row(src, row.unwrap_or_default() as i64, is_record) }
}

/// Narrow a Vec to the `[lo, hi)` window, clamping out-of-range bounds jq-style rather than
/// answering absence as `tl_at` does: negatives count from the end, both bounds clamp to
/// `[0, len]`, and a crossed window is empty (kantord/toylang#143). A bound left out arrives as
/// `i64::MIN` for the start or `i64::MAX` for the end, each folding to the Vec's own boundary
/// under the clamp. `depth` is as for `tl_at`. The window is copied, not shared.
///
/// # Safety
/// As `tl_at`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_vec_slice(v: *const TlVec, lo: i64, hi: i64, depth: i64) -> *mut TlVec {
    if depth > 0 {
        let out = tl_vec_new(unsafe { (*v).len }, 1);
        let inner = unsafe { column(v, 0) };
        for (dst, &slot) in unsafe { column_mut(out, 0) }.iter_mut().zip(inner) {
            *dst = unsafe { tl_vec_slice(slot as *const TlVec, lo, hi, depth - 1) } as i64;
        }
        return out;
    }
    let (n, ncols) = unsafe { ((*v).len, (*v).ncols) };
    // A negative bound is at least `-len` from the end or clamps to zero, so `b + n` cannot
    // overflow.
    let clamp = |b: i64| (if b < 0 { b + n } else { b }).clamp(0, n);
    let (lo, hi) = (clamp(lo), clamp(hi));
    let len = (hi - lo).max(0);
    let out = tl_vec_new(len, ncols);
    for c in 0..ncols {
        let window = &unsafe { column(v, c) }[lo as usize..(lo + len) as usize];
        unsafe { column_mut(out, c) }.copy_from_slice(window);
    }
    out
}

/// Insist an Opt is present, `depth` layers down. Needs no `is_record` flag: an Opt already
/// holds a gathered value, so there is nothing left to collect out of columns.
///
/// # Safety
/// `o` is an Opt from `tl_opt_some` or null, or at depth above zero a Vec of them.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_unwrap(o: *mut i64, depth: i64) -> *mut i64 {
    if depth > 0 {
        let v = o as *const TlVec;
        let out = tl_vec_new(unsafe { (*v).len }, 1);
        let inner = unsafe { column(v, 0) };
        for (dst, &slot) in unsafe { column_mut(out, 0) }.iter_mut().zip(inner) {
            *dst = unsafe { tl_unwrap(slot as *mut i64, depth - 1) } as i64;
        }
        return out.cast();
    }
    if o.is_null() {
        fail("unwrapped a value that is not there");
    }
    unsafe { *o as *mut i64 }
}

/// The integers from zero up to but not including `n`. A negative `n` gives an empty Vec, the
/// same as asking for zero of them.
#[unsafe(no_mangle)]
pub extern "C" fn tl_range(n: i64) -> *mut TlVec {
    let out = tl_vec_new(n.max(0), 1);
    for (i, slot) in unsafe { column_mut(out, 0) }.iter_mut().enumerate() {
        *slot = i as i64;
    }
    out
}

/// Every Unicode scalar value in `s`, one codepoint per element: not a byte and not a UTF-16
/// unit, so a character outside the Basic Multilingual Plane is one element here even where a
/// backend with UTF-16 strings spells it with a surrogate pair. Every Str the runtime builds is
/// valid UTF-8 (input is refused otherwise, and JSON's lone surrogates are), so the lossy decode
/// only decides what happens if that ever stops holding: U+FFFD, where the C decoder read past
/// the end of the string.
///
/// # Safety
/// `s` points at a live Str whose bytes are valid for its length.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_chars(s: *const TlStr) -> *mut TlVec {
    let text = String::from_utf8_lossy(unsafe { bytes(s) });
    let out = tl_vec_new(text.chars().count() as i64, 1);
    for (slot, ch) in unsafe { column_mut(out, 0) }.iter_mut().zip(text.chars()) {
        *slot = ch as i64;
    }
    out
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

/// Split one Str on a literal delimiter: every occurrence, in order, with the empty string one
/// empty field and a trailing delimiter a trailing empty field, the same shape `str.split` gives
/// on every other backend. The delimiter is searched literally, never as a pattern.
///
/// An empty delimiter is unreachable (the checker refuses `dsv("")`); `str::split("")` would give
/// a field per character with an empty one at each end, where the C loop never advanced.
fn split(s: &[u8], sep: &[u8]) -> *mut TlVec {
    let (Ok(s), Ok(sep)) = (std::str::from_utf8(s), std::str::from_utf8(sep)) else {
        fail("split of a Str that is not valid UTF-8");
    };
    let fields: Vec<i64> = s
        .split(sep)
        .map(|field| leak_str(field.as_bytes().to_vec()) as i64)
        .collect();
    vec_of_slots(&fields)
}

/// Split every line of a Vec<Str> on the delimiter, one row per line: `dsv(delim)`. The outer Vec
/// is a single column of inner Vecs, the struct-of-arrays spelling of Vec<Vec<Str>>.
///
/// # Safety
/// `lines` is a live one-column `TlVec` of Strs and `sep` a live Str.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_split_lines(lines: *const TlVec, sep: *const TlStr) -> *mut TlVec {
    let sep = unsafe { bytes(sep) };
    let rows: Vec<i64> = unsafe { column(lines, 0) }
        .iter()
        .map(|&line| split(unsafe { bytes(line as *const TlStr) }, sep) as i64)
        .collect();
    vec_of_slots(&rows)
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

    /// A Vec<Vec<T>> whose inner Vecs are `rows`, each a set of columns.
    unsafe fn nested(rows: &[*mut TlVec]) -> *mut TlVec {
        let slots: Vec<i64> = rows.iter().map(|&r| r as i64).collect();
        unsafe { vec_of(&[&slots]) }
    }

    #[test]
    fn reverse_flips_every_column_together() {
        unsafe {
            let v = vec_of(&[&[1, 2, 3], &[10, 20, 30]]);
            let out = tl_vec_reverse(v, 2);
            assert_eq!(column_of(out, 0), [3, 2, 1]);
            assert_eq!(column_of(out, 1), [30, 20, 10]);
            assert_eq!(column_of(v, 0), [1, 2, 3]);
            let empty = tl_vec_reverse(tl_vec_new(0, 2), 2);
            assert_eq!((tl_vec_len(empty), (*empty).ncols), (0, 2));
        }
    }

    #[test]
    fn flatten_skips_empty_inners_and_keeps_the_width_of_an_empty_outer() {
        unsafe {
            let inner = [
                vec_of(&[&[1, 2], &[10, 20]]),
                tl_vec_new(0, 2),
                vec_of(&[&[3], &[30]]),
            ];
            let out = tl_vec_flatten(nested(&inner), 2);
            assert_eq!(column_of(out, 0), [1, 2, 3]);
            assert_eq!(column_of(out, 1), [10, 20, 30]);

            let none = tl_vec_flatten(nested(&[]), 3);
            assert_eq!((tl_vec_len(none), (*none).ncols), (0, 3));
        }
    }

    #[test]
    fn concat_joins_either_side_empty() {
        unsafe {
            let a = vec_of(&[&[1, 2], &[10, 20]]);
            let b = vec_of(&[&[3], &[30]]);
            let out = tl_vec_concat(a, b, 2);
            assert_eq!(column_of(out, 0), [1, 2, 3]);
            assert_eq!(column_of(out, 1), [10, 20, 30]);
            let left = tl_vec_concat(tl_vec_new(0, 2), b, 2);
            assert_eq!(column_of(left, 1), [30]);
            let right = tl_vec_concat(a, tl_vec_new(0, 2), 2);
            assert_eq!(column_of(right, 0), [1, 2]);
            let neither = tl_vec_concat(tl_vec_new(0, 2), tl_vec_new(0, 2), 2);
            assert_eq!((tl_vec_len(neither), (*neither).ncols), (0, 2));
        }
    }

    #[test]
    fn transpose_swaps_rows_and_columns_of_every_field() {
        unsafe {
            let rows = [
                vec_of(&[&[1, 2, 3], &[10, 20, 30]]),
                vec_of(&[&[4, 5, 6], &[40, 50, 60]]),
            ];
            let out = tl_vec_transpose(nested(&rows), 2);
            assert_eq!(tl_vec_len(out), 3);
            let col1 = *column(out, 0).get(1).unwrap() as *const TlVec;
            assert_eq!(column_of(col1, 0), [2, 5]);
            assert_eq!(column_of(col1, 1), [20, 50]);

            // Zero-width rows and no rows both give an empty Vec of Vecs, one column wide.
            let flat = tl_vec_transpose(nested(&[tl_vec_new(0, 1), tl_vec_new(0, 1)]), 1);
            assert_eq!((tl_vec_len(flat), (*flat).ncols), (0, 1));
            let none = tl_vec_transpose(nested(&[]), 1);
            assert_eq!((tl_vec_len(none), (*none).ncols), (0, 1));
        }
    }

    #[test]
    fn tail_first_any_all_on_empty_and_full() {
        unsafe {
            let v = vec_of(&[&[1, 2, 3], &[10, 20, 30]]);
            let tail = tl_opt_get(tl_vec_tail(v)) as *const TlVec;
            assert_eq!(column_of(tail, 0), [2, 3]);
            assert_eq!(column_of(tail, 1), [20, 30]);
            let single = tl_opt_get(tl_vec_tail(vec_of(&[&[7]]))) as *const TlVec;
            assert_eq!(tl_vec_len(single), 0);
            assert!(tl_vec_tail(tl_vec_new(0, 2)).is_null());

            assert_eq!(tl_opt_get(tl_vec_first(v, 0)), 1);
            let rec = tl_opt_get(tl_vec_first(v, 1)) as *const i64;
            assert_eq!((tl_rec_get(rec, 0), tl_rec_get(rec, 1)), (1, 10));
            assert!(tl_vec_first(tl_vec_new(0, 1), 0).is_null());

            let (none, some, mixed) = (vec_of(&[&[0, 0]]), vec_of(&[&[1, 1]]), vec_of(&[&[0, 1]]));
            assert_eq!(
                (tl_vec_any(none), tl_vec_any(some), tl_vec_any(mixed)),
                (0, 1, 1)
            );
            assert_eq!(
                (tl_vec_all(none), tl_vec_all(some), tl_vec_all(mixed)),
                (0, 1, 0)
            );
            let empty = tl_vec_new(0, 1);
            assert_eq!((tl_vec_any(empty), tl_vec_all(empty)), (0, 1));
        }
    }

    #[test]
    fn sum_wraps_to_32_bits_only_for_int() {
        unsafe {
            let v = vec_of(&[&[i32::MAX as i64, 1]]);
            assert_eq!(tl_vec_sum(v, 1), i32::MIN as i64);
            assert_eq!(tl_vec_sum(v, 0), i32::MAX as i64 + 1);
            let big = vec_of(&[&[i64::MAX, 1]]);
            assert_eq!(tl_vec_sum(big, 0), i64::MIN, "Int64 wraps at 64 bits");
            // Each addition narrows, not just the last: 2^31 - 1 + 1 + 1 wraps once, then adds.
            assert_eq!(
                tl_vec_sum(vec_of(&[&[i32::MAX as i64, 1, 1]]), 1),
                i32::MIN as i64 + 1
            );
            assert_eq!(tl_vec_sum(tl_vec_new(0, 1), 1), 0);
        }
    }

    #[test]
    fn max_of_ints_and_of_nothing() {
        unsafe {
            assert_eq!(tl_opt_get(tl_vec_max(vec_of(&[&[-5, i64::MIN, -2]]))), -2);
            assert!(tl_vec_max(tl_vec_new(0, 1)).is_null());
        }
    }

    #[test]
    fn at_counts_from_the_end_and_answers_absence() {
        unsafe {
            let v = vec_of(&[&[10, 20, 30], &[1, 2, 3]]);
            let at = |i| tl_at(v, i, 0, 0);
            assert_eq!(tl_opt_get(at(0)), 10);
            assert_eq!(tl_opt_get(at(-1)), 30);
            assert_eq!(tl_opt_get(at(-3)), 10);
            assert!(at(3).is_null() && at(-4).is_null() && at(i64::MIN).is_null());
            let rec = tl_opt_get(tl_at(v, 1, 0, 1)) as *const i64;
            assert_eq!((tl_rec_get(rec, 0), tl_rec_get(rec, 1)), (20, 2));
            assert!(tl_at(tl_vec_new(0, 1), 0, 0, 0).is_null());

            // One layer down: the index applies inside each inner Vec.
            let rows = nested(&[vec_of(&[&[1, 2]]), vec_of(&[&[3]])]);
            let out = tl_at(rows, -1, 1, 0) as *const TlVec;
            assert_eq!(tl_vec_len(out), 2);
            let opts = column_of(out, 0);
            assert_eq!(tl_opt_get(opts[0] as *const i64), 2);
            assert_eq!(tl_opt_get(opts[1] as *const i64), 3);
        }
    }

    #[test]
    fn select_reads_through_the_mask() {
        unsafe {
            let src = vec_of(&[&[10, 11, 12, 13, 14], &[0, 1, 2, 3, 4]]);
            let keep = tl_mask_new(5);
            for (i, k) in [0, 1, 0, 1, 1].into_iter().enumerate() {
                tl_mask_set(keep, i as i64, k);
            }
            assert_eq!(tl_sel_len(src, keep), 3);
            let at = |i| tl_sel_at(src, keep, i, 0);
            assert_eq!(
                (tl_opt_get(at(0)), tl_opt_get(at(1)), tl_opt_get(at(2))),
                (11, 13, 14)
            );
            assert_eq!(tl_opt_get(at(-1)), 14);
            assert!(at(3).is_null() && at(-4).is_null());
            let rec = tl_opt_get(tl_sel_at(src, keep, 1, 1)) as *const i64;
            assert_eq!((tl_rec_get(rec, 0), tl_rec_get(rec, 1)), (13, 3));

            let empty = tl_vec_new(0, 1);
            assert_eq!(tl_sel_len(empty, tl_mask_new(0)), 0);
            assert!(tl_sel_at(empty, tl_mask_new(0), 0, 0).is_null());
        }
    }

    #[test]
    fn slice_clamps_like_jq_and_copies_the_window() {
        unsafe {
            let v = vec_of(&[&[0, 1, 2, 3, 4], &[10, 11, 12, 13, 14]]);
            let cut = |lo, hi| column_of(tl_vec_slice(v, lo, hi, 0), 0);
            assert_eq!(cut(1, 3), [1, 2]);
            assert_eq!(cut(-2, i64::MAX), [3, 4]);
            assert_eq!(cut(i64::MIN, -3), [0, 1]);
            assert_eq!(cut(-99, 99), [0, 1, 2, 3, 4]);
            assert_eq!(cut(4, 1), [] as [i64; 0], "a crossed window is empty");
            assert_eq!(cut(99, 100), [] as [i64; 0]);
            assert_eq!(cut(i64::MIN, i64::MIN), [] as [i64; 0]);

            let both = tl_vec_slice(v, 2, 4, 0);
            assert_eq!(column_of(both, 1), [12, 13]);
            tl_vec_set(both, 0, 0, 99);
            assert_eq!(tl_vec_get(v, 0, 2), 2, "the window is a copy");

            let empty = tl_vec_slice(tl_vec_new(0, 2), i64::MIN, i64::MAX, 0);
            assert_eq!((tl_vec_len(empty), (*empty).ncols), (0, 2));

            let rows = nested(&[v, vec_of(&[&[7], &[8]])]);
            let out = tl_vec_slice(rows, 1, 2, 1);
            let inner = column_of(out, 0);
            assert_eq!(column_of(inner[0] as *const TlVec, 0), [1]);
            assert_eq!(column_of(inner[1] as *const TlVec, 0), [] as [i64; 0]);
        }
    }

    #[test]
    fn unwrap_peels_one_opt_per_layer() {
        unsafe {
            assert_eq!(tl_unwrap(tl_opt_some(5), 0) as i64, 5);
            let opts = vec_of(&[&[tl_opt_some(1) as i64, tl_opt_some(2) as i64]]);
            let out = tl_unwrap(opts.cast(), 1) as *const TlVec;
            assert_eq!(column_of(out, 0), [1, 2]);
            let none = tl_unwrap(tl_vec_new(0, 1).cast(), 1) as *const TlVec;
            assert_eq!(tl_vec_len(none), 0);
        }
    }

    #[test]
    fn range_counts_up_and_treats_negative_as_zero() {
        unsafe {
            assert_eq!(column_of(tl_range(4), 0), [0, 1, 2, 3]);
            assert_eq!(tl_vec_len(tl_range(0)), 0);
            assert_eq!(tl_vec_len(tl_range(-7)), 0);
            assert_eq!(tl_vec_len(tl_range(i64::MIN)), 0);
        }
    }

    #[test]
    fn chars_are_codepoints_not_bytes_or_utf16_units() {
        unsafe {
            let of = |text: &str| column_of(tl_chars(leak_str(text.as_bytes().to_vec())), 0);
            assert_eq!(of("abc"), [97, 98, 99]);
            assert_eq!(of(""), [] as [i64; 0]);
            assert_eq!(of("\u{e9}\u{20ac}\u{1f600}"), [0xe9, 0x20ac, 0x1f600]);
            // Not valid UTF-8, which no Str the runtime builds is: decoded, not read past.
            let bad = leak_str(vec![b'a', 0xe2, 0x82]);
            assert_eq!(column_of(tl_chars(bad), 0), [97, 0xfffd]);
        }
    }

    unsafe fn fields_of(v: *const TlVec) -> Vec<String> {
        unsafe { column(v, 0) }
            .iter()
            .map(|&s| String::from_utf8(unsafe { bytes(s as *const TlStr) }.to_vec()).unwrap())
            .collect()
    }

    #[test]
    fn split_keeps_empty_and_trailing_fields() {
        let cut = |s: &str, sep: &str| unsafe { fields_of(split(s.as_bytes(), sep.as_bytes())) };
        assert_eq!(cut("a,b,c", ","), ["a", "b", "c"]);
        assert_eq!(cut("", ","), [""], "the empty Str is one empty field");
        assert_eq!(
            cut("a,", ","),
            ["a", ""],
            "a trailing delimiter is a trailing field"
        );
        assert_eq!(cut(",,", ","), ["", "", ""]);
        assert_eq!(
            cut("a::b:c", "::"),
            ["a", "b:c"],
            "the delimiter is literal, and can be long"
        );
        assert_eq!(cut(".*", ".*"), ["", ""], "never a pattern");
        assert_eq!(cut("h\u{e9},\u{1f600}", ","), ["h\u{e9}", "\u{1f600}"]);
    }

    #[test]
    fn split_lines_makes_a_row_per_line() {
        unsafe {
            let lines = vec_of_slots(&[str_slot("a\tb"), str_slot(""), str_slot("c")]);
            let out = tl_split_lines(lines, leak_str(b"\t".to_vec()));
            let rows: Vec<Vec<String>> = column(out, 0)
                .iter()
                .map(|&r| fields_of(r as *const TlVec))
                .collect();
            assert_eq!(rows, [vec!["a", "b"], vec![""], vec!["c"]]);
            assert_eq!(
                tl_vec_len(tl_split_lines(tl_vec_new(0, 1), leak_str(vec![b',']))),
                0
            );
        }
    }
}
