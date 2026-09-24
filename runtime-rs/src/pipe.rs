//! `pipe_through`: run a command with lines on its stdin and collect what it prints.

use std::ffi::OsStr;
use std::io::{Read, Write};
use std::os::unix::ffi::OsStrExt;
use std::process::{Command, Stdio};
use std::ptr::null_mut;
use std::thread::{Builder, Scope, ScopedJoinHandle};

use super::{TlStr, TlVec, bytes, column, fail, fail_at, leak_str, tl_rec_new, vec_of_slots};

/// The OS's own message for a failed spawn, without the ` (os error N)` that `io::Error` appends,
/// so it reads as `strerror` did in the C.
fn os_message(e: &std::io::Error) -> String {
    let text = e.to_string();
    match e.raw_os_error() {
        Some(code) => text
            .strip_suffix(&format!(" (os error {code})"))
            .unwrap_or(&text)
            .to_string(),
        None => text,
    }
}

/// Runs `f` on a scoped thread; failing to get a thread is a runtime failure, not a panic.
fn spawn_thread<'scope, 'env, T: Send + 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    f: impl FnOnce() -> T + Send + 'scope,
) -> ScopedJoinHandle<'scope, T> {
    Builder::new()
        .spawn_scoped(scope, f)
        .unwrap_or_else(|e| fail(&format!("cannot start a thread for pipe_through: {e}")))
}

/// Writes `input` to the child's stdin and closes it. Runs on its own thread so a child that has
/// not read yet cannot keep the program from draining its output, and one that never reads
/// cannot stall it past the child's own exit.
///
/// SIGPIPE is blocked on this thread only. The generated `main` does not go through Rust's
/// `lang_start`, so SIGPIPE keeps its default action and a write to a child that already closed
/// its stdin (`head`, `sort -u`) would kill the program instead of failing with EPIPE. Blocking
/// it here, rather than ignoring it process-wide as the C did around its poll loop, leaves the
/// program's own disposition alone: a closed stdout of its own still ends it, as for any filter.
/// The signal a blocked write leaves pending dies with the thread.
fn feed(mut stdin: impl Write, input: &[u8]) {
    unsafe {
        let mut set: libc::sigset_t = std::mem::zeroed();
        libc::sigemptyset(&mut set);
        libc::sigaddset(&mut set, libc::SIGPIPE);
        libc::pthread_sigmask(libc::SIG_BLOCK, &set, null_mut());
    }
    // A write error is the child having closed stdin early, which is normal; whatever else it
    // was, the child's own output is still the result.
    let _ = stdin.write_all(input);
}

/// Everything the pipe yields until EOF. A read error ends it there, as the C poll loop did.
fn drain(mut pipe: impl Read) -> Vec<u8> {
    let mut buf = Vec::new();
    let _ = pipe.read_to_end(&mut buf);
    buf
}

/// Splits on `\n` only, keeping a `\r`, and pushes each line as a PipeLine box: slot 0 the
/// variant tag, slot 1 a one-field record holding the text, the same layout the emitter builds
/// for `Stdout{text}`. A final line with no newline is still a line; empty output has none.
fn push_lines(items: &mut Vec<i64>, data: &[u8], tag: i64) {
    if data.is_empty() {
        return;
    }
    if std::str::from_utf8(data).is_err() {
        fail_at("subprocess output is not valid UTF-8", "pipe_through");
    }
    let body = data.strip_suffix(b"\n").unwrap_or(data);
    for line in body.split(|&b| b == b'\n') {
        let payload = tl_rec_new(1);
        let boxed = tl_rec_new(2);
        unsafe {
            *payload = leak_str(line.to_vec()) as i64;
            *boxed = tag;
            *boxed.add(1) = payload as i64;
        }
        items.push(boxed as i64);
    }
}

/// Runs `cmd args` with `lines` on its stdin, one per line, and returns the lines it printed on
/// stdout and then those on stderr, each as a PipeLine box tagged `stdout_tag` or `stderr_tag`.
/// The exit status is ignored, and the arguments reach the child as argv, never through a shell.
///
/// # Safety
/// `cmd` is a live Str, `args` and `lines` are live one-column Vecs of Strs.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tl_pipe_through(
    cmd: *const TlStr,
    args: *const TlVec,
    lines: *const TlVec,
    stdout_tag: i64,
    stderr_tag: i64,
) -> *mut TlVec {
    let cmd = unsafe { bytes(cmd) };
    let mut input = Vec::new();
    for &line in unsafe { column(lines, 0) } {
        input.extend_from_slice(unsafe { bytes(line as *const TlStr) });
        input.push(b'\n');
    }

    let mut child = Command::new(OsStr::from_bytes(cmd))
        .args(
            unsafe { column(args, 0) }
                .iter()
                .map(|&a| OsStr::from_bytes(unsafe { bytes(a as *const TlStr) })),
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| {
            fail(&format!(
                "cannot spawn subprocess `{}`: {}",
                String::from_utf8_lossy(cmd),
                os_message(&e)
            ))
        });

    // Three pipes are drained or fed at once, because a child blocked on a full one would
    // otherwise wait for a side that is blocked on another: stdin on a thread, stderr on a
    // thread, stdout here. `wait_with_output` closes stdin before it reads, which is no use to a
    // child that reads its stdin while writing.
    let (stdin, stdout, stderr) = (
        child.stdin.take().expect("stdin was piped"),
        child.stdout.take().expect("stdout was piped"),
        child.stderr.take().expect("stderr was piped"),
    );
    let (out, err) = std::thread::scope(|scope| {
        spawn_thread(scope, || feed(stdin, &input));
        let err = spawn_thread(scope, || drain(stderr));
        let out = drain(stdout);
        (out, err.join().expect("the stderr reader does not panic"))
    });
    let _ = child.wait();

    let mut items = Vec::new();
    push_lines(&mut items, &out, stdout_tag);
    push_lines(&mut items, &err, stderr_tag);
    vec_of_slots(&items)
}

/// Real children, so `sh`, `cat`, `head`, `seq` and `echo` must be on the PATH. Note the test
/// binary's `main` does go through `lang_start`, which ignores SIGPIPE: what a generated program
/// does when SIGPIPE has its default action is checked in tests/native_pipe_through.rs.
#[cfg(test)]
mod tests {
    use super::*;

    fn strs(items: &[&str]) -> *mut TlVec {
        let slots: Vec<i64> = items
            .iter()
            .map(|s| leak_str(s.as_bytes().to_vec()) as i64)
            .collect();
        vec_of_slots(&slots)
    }

    /// `(tag, text)` of every line `tl_pipe_through` returned.
    fn run(cmd: &str, args: &[&str], lines: &[&str]) -> Vec<(i64, String)> {
        unsafe {
            let out = tl_pipe_through(
                leak_str(cmd.as_bytes().to_vec()),
                strs(args),
                strs(lines),
                7,
                8,
            );
            column(out, 0)
                .iter()
                .map(|&boxed| {
                    let boxed = boxed as *const i64;
                    let text = *(*boxed.add(1) as *const i64) as *const TlStr;
                    (*boxed, String::from_utf8(bytes(text).to_vec()).unwrap())
                })
                .collect()
        }
    }

    fn line(tag: i64, text: &str) -> (i64, String) {
        (tag, text.to_string())
    }

    #[test]
    fn stdout_lines_come_before_stderr_lines_each_with_its_tag() {
        let got = run(
            "sh",
            &["-c", "cat; echo one >&2; echo two >&2"],
            &["a", "b"],
        );
        assert_eq!(
            got,
            [line(7, "a"), line(7, "b"), line(8, "one"), line(8, "two")]
        );
    }

    #[test]
    fn lines_keep_a_carriage_return_and_a_last_line_needs_no_newline() {
        let got = run("printf", &["a\\r\\n\\nb"], &[]);
        assert_eq!(got, [line(7, "a\r"), line(7, ""), line(7, "b")]);
    }

    #[test]
    fn no_output_is_no_lines() {
        assert_eq!(run("true", &[], &["ignored"]), []);
        assert_eq!(
            run("grep", &["zzz"], &["a"]),
            [],
            "exit status 1 is not an error"
        );
    }

    #[test]
    fn every_input_line_gets_a_newline() {
        assert_eq!(
            run("cat", &[], &["a", "", "b\r"]),
            [line(7, "a"), line(7, ""), line(7, "b\r")]
        );
    }

    #[test]
    fn arguments_are_argv_not_shell_text() {
        assert_eq!(
            run("echo", &["$HOME", "*", "it's"], &[]),
            [line(7, "$HOME * it's")]
        );
    }

    #[test]
    fn a_child_that_never_reads_stdin_does_not_stall() {
        let lines: Vec<String> = (0..200_000).map(|i| i.to_string()).collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(run("echo", &["hi"], &lines), [line(7, "hi")]);
    }

    #[test]
    fn a_child_that_stops_reading_early_is_not_an_error() {
        let lines: Vec<String> = (0..200_000).map(|i| i.to_string()).collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        assert_eq!(
            run("head", &["-n", "2"], &lines),
            [line(7, "0"), line(7, "1")]
        );
    }

    /// The child fills stderr while the parent would be waiting on stdout, and the input is
    /// larger than a pipe buffer while the child writes: any single-threaded drain deadlocks.
    #[test]
    fn large_output_on_both_pipes_while_input_is_pending_does_not_deadlock() {
        let lines: Vec<String> = (0..100_000).map(|i| i.to_string()).collect();
        let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
        let got = run("sh", &["-c", "seq 1 100000 >&2; cat"], &lines);
        let on = |tag| got.iter().filter(|(t, _)| *t == tag).count();
        assert_eq!((on(7), on(8)), (100_000, 100_000));
        assert_eq!(got[0], line(7, "0"), "stdout lines first");
    }

    #[test]
    fn a_line_that_is_only_a_newline_is_one_empty_line() {
        let mut items = Vec::new();
        push_lines(&mut items, b"\n", 1);
        push_lines(&mut items, b"a\n\n", 1);
        assert_eq!(items.len(), 3);
    }

    #[test]
    fn a_spawn_error_reads_as_the_os_message() {
        let e = Command::new("/nonexistent/toylang-test-cmd")
            .spawn()
            .unwrap_err();
        assert_eq!(os_message(&e), "No such file or directory");
    }
}
