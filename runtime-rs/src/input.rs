//! Reading the program's input from stdin.

use std::io::BufRead;

use super::{TlVec, fail_at, leak_str, vec_of_slots};

/// Appends the next line of stdin to `buf` and returns whether there was one. The `\n` is
/// stripped and a `\r` before it is kept (`jq -R` and Python's stdin iteration do the same); a
/// final line with no newline is still a line; a blank line is one too, since `lines` keeps
/// them. A line that is not UTF-8 is refused rather than carried, because a Str is Unicode
/// scalar values (kantord/toylang#102).
fn read_line(buf: &mut Vec<u8>) -> bool {
    let start = buf.len();
    match std::io::stdin().lock().read_until(b'\n', buf) {
        Ok(0) => return false,
        Ok(_) => {}
        Err(_) => fail_at("could not read stdin", ""),
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
