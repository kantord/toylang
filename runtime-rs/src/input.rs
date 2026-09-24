//! Reading the program's input from stdin.

use std::io::{BufRead, Read};

use super::json::{Failure, Reader, Schema};
use super::{TlStr, TlVec, bytes, fail_at, leak_str, vec_of_slots};

fn or_fail<T>(result: Result<T, Failure>) -> T {
    result.unwrap_or_else(|f| fail_at(&f.what, &f.path))
}

/// Appends the next line of stdin to `buf`, its `\n` included, and returns whether there was one.
fn read_raw_line(buf: &mut Vec<u8>) -> bool {
    match std::io::stdin().lock().read_until(b'\n', buf) {
        Ok(0) => false,
        Ok(_) => true,
        Err(_) => fail_at("could not read stdin", ""),
    }
}

/// Appends the next line of stdin to `buf` and returns whether there was one. The `\n` is
/// stripped and a `\r` before it is kept (`jq -R` and Python's stdin iteration do the same); a
/// final line with no newline is still a line; a blank line is one too, since `lines` keeps
/// them. A line that is not UTF-8 is refused rather than carried, because a Str is Unicode
/// scalar values (kantord/toylang#102).
fn read_line(buf: &mut Vec<u8>) -> bool {
    let start = buf.len();
    if !read_raw_line(buf) {
        return false;
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
    }
    if std::str::from_utf8(&buf[start..]).is_err() {
        fail_at("stdin is not valid UTF-8", "lines");
    }
    true
}

/// One raw line of stdin per call, the streaming counterpart of `tl_collect_lines`: 0 at EOF
/// (`*out` untouched), else 1 with `*out` set to the line's Str.
///
/// # Safety
/// `out` is valid for one write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_read_one_line(out: *mut i64) -> i32 {
    let mut line = Vec::new();
    if !read_line(&mut line) {
        return 0;
    }
    unsafe { *out = leak_str(line) as i64 };
    1
}

/// Every remaining line of stdin, as a Vec<Str>.
#[unsafe(no_mangle)]
pub extern "C" fn tl_collect_lines() -> *mut TlVec {
    let mut lines = Vec::new();
    let mut buf = Vec::new();
    while {
        buf.clear();
        read_line(&mut buf)
    } {
        lines.push(leak_str(buf.clone()) as i64);
    }
    vec_of_slots(&lines)
}

/// All of stdin, one JSON value, parsed against the descriptor. A refusal names `input`.
///
/// # Safety
/// `descriptor` is a live Str.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_read_input(descriptor: *const TlStr) -> i64 {
    let mut text = Vec::new();
    if std::io::stdin().lock().read_to_end(&mut text).is_err() {
        fail_at("could not read stdin", "");
    }
    let schema = or_fail(Schema::parse(unsafe { bytes(descriptor) }, "input"));
    or_fail(Reader::new(&schema, "input").document(&text))
}

/// `parse(s)`: one JSON value read from the string in hand, not from stdin, against the same
/// descriptor. A refusal names `parse`.
///
/// # Safety
/// `value` and `descriptor` are live Strs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_parse_str(value: *const TlStr, descriptor: *const TlStr) -> i64 {
    let schema = or_fail(Schema::parse(unsafe { bytes(descriptor) }, "parse"));
    or_fail(Reader::new(&schema, "parse").document(unsafe { bytes(value) }))
}

/// A line of a stream with nothing on it but whitespace is not a value, and is skipped.
fn is_blank(line: &[u8]) -> bool {
    line.iter()
        .all(|b| matches!(b, b' ' | b'\t' | b'\n' | b'\r'))
}

/// The next non-blank line of stdin, read as one value of the stream. A refusal names `inputs`.
fn read_value(reader: &Reader, line: &mut Vec<u8>) -> Option<i64> {
    loop {
        line.clear();
        if !read_raw_line(line) {
            return None;
        }
        if !is_blank(line) {
            return Some(or_fail(reader.document(line)));
        }
    }
}

/// Every remaining JSON value on stdin, one per line, assembled into a Vec (spread into columns
/// when the element is a record, the invariant `vec_lit` and `tl_at` already keep).
///
/// # Safety
/// `descriptor` is a live Str.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_read_inputs(descriptor: *const TlStr) -> *mut TlVec {
    let schema = or_fail(Schema::parse(unsafe { bytes(descriptor) }, "inputs"));
    let reader = Reader::new(&schema, "inputs");
    let mut items = Vec::new();
    let mut line = Vec::new();
    while let Some(value) = read_value(&reader, &mut line) {
        items.push(value);
    }
    schema.vec_of(&items)
}

/// One JSON value from stdin per call, read the way `tl_read_inputs` reads each of its own but
/// with no Vec ever built: the caller drives its own loop and decides when to stop, which is what
/// lets the native backend fuse `jsonlines(f(inputs))` into a read-one/transform-one/write-one
/// loop (see tir::fusion). Returns 0 at EOF (`*out` untouched) and 1 with `*out` set otherwise: a
/// separate flag, because a record pointer or an Int can legitimately be any i64.
///
/// # Safety
/// `descriptor` is a live Str and `out` is valid for one write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_read_one_input(descriptor: *const TlStr, out: *mut i64) -> i32 {
    let schema = or_fail(Schema::parse(unsafe { bytes(descriptor) }, "inputs"));
    let reader = Reader::new(&schema, "inputs");
    let Some(value) = read_value(&reader, &mut Vec::new()) else {
        return 0;
    };
    unsafe { *out = value };
    1
}
