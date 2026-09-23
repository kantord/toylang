//! `pipe_through` runs on every backend that can express it, and they agree. It is not a corpus
//! case: the corpus needs all seven backends to agree, and jq cannot spawn a process
//! (`backend_support::Landing::never_on`), so the corpus has no per-case exemption to offer it.
//! This file is that exemption written down: every backend `LANDINGS` says has an arm runs each
//! program, and jq is held to its refusal in `tests/unbuilt_arms.rs`.

use toylang::Backend;
use toylang::backend_support::LANDINGS;

fn backends_with_an_arm() -> &'static [Backend] {
    LANDINGS
        .iter()
        .find(|l| l.name == "pipe_through")
        .expect("pipe_through is a landing")
        .built_on
}

fn agree(program: &str, input: &str) -> String {
    let mut results = backends_with_an_arm()
        .iter()
        .map(|&b| (b, toylang::run_on(program, Some(input), b)));
    let (first, expected) = results.next().unwrap();
    let expected = expected.unwrap_or_else(|e| panic!("{} failed: {e}", first.name()));
    for (backend, result) in results {
        let got = result.unwrap_or_else(|e| panic!("{} failed: {e}", backend.name()));
        assert_eq!(
            got,
            expected,
            "{} disagrees with {}",
            backend.name(),
            first.name()
        );
    }
    expected
}

/// The stdout lines, then the stderr lines, each tagged: the order is what makes the output
/// reproducible when a child writes to both.
#[test]
fn stdout_lines_come_before_stderr_lines() {
    let out = agree(
        "collect(pipe_through({cmd: \"sh\", args: [\"-c\", \"cat; echo one >&2; echo two >&2\"], lines: stdin}))\n",
        "a\nb\n",
    );
    assert_eq!(
        out.trim(),
        r#"[{"Stdout":{"text":"a"}},{"Stdout":{"text":"b"}},{"Stderr":{"text":"one"}},{"Stderr":{"text":"two"}}]"#
    );
}

/// Exit status is not an error: `grep` with no match exits 1.
#[test]
fn a_nonzero_exit_is_a_normal_outcome() {
    let out = agree(
        "collect(pipe_through({cmd: \"grep\", args: [\"zzz\"], lines: stdin}))\n",
        "a\nb\n",
    );
    assert_eq!(out.trim(), "[]");
}

/// A child that never reads its stdin must not stall the program, even when there is far more
/// input than a pipe buffer holds.
#[test]
fn a_child_that_never_reads_stdin_does_not_stall() {
    let input: String = (0..200_000).map(|i| format!("{i}\n")).collect();
    let out = agree(
        "collect(pipe_through({cmd: \"echo\", args: [\"hi\"], lines: stdin}))\n",
        &input,
    );
    assert_eq!(out.trim(), r#"[{"Stdout":{"text":"hi"}}]"#);
}

/// A child that closes its stdin early (`head`) is normal, not an error, however much input is
/// still waiting to be written.
#[test]
fn a_child_that_stops_reading_early_is_not_an_error() {
    let input: String = (0..200_000).map(|i| format!("{i}\n")).collect();
    let out = agree(
        "collect(pipe_through({cmd: \"head\", args: [\"-n\", \"2\"], lines: stdin}))\n",
        &input,
    );
    assert_eq!(
        out.trim(),
        r#"[{"Stdout":{"text":"0"}},{"Stdout":{"text":"1"}}]"#
    );
}

/// Output larger than a pipe buffer on both pipes at once is the deadlock the concurrent drain
/// exists for: a child filling stderr while the parent waits on stdout.
#[test]
fn large_output_on_both_pipes_does_not_deadlock() {
    let out = agree(
        "collect(pipe_through({cmd: \"sh\", args: [\"-c\", \"seq 1 100000; seq 1 100000 >&2\"], lines: stdin})) | length(.)\n",
        "",
    );
    assert_eq!(out.trim(), "200000");
}

/// `cmd` and `args` are argv, not shell text.
#[test]
fn arguments_are_not_shell_expanded() {
    let out = agree(
        "collect(pipe_through({cmd: \"echo\", args: [\"$HOME\", \"*\", \"it's\"], lines: stdin}))\n",
        "",
    );
    assert_eq!(out.trim(), r#"[{"Stdout":{"text":"$HOME * it's"}}]"#);
}

/// Lines split on `\n` only and keep a `\r`, the way `lines` does. Go's `bufio.Scanner` and
/// Python's text mode each dropped it before this test.
#[test]
fn lines_keep_a_carriage_return() {
    let out = agree(
        "collect(pipe_through({cmd: \"printf\", args: [\"a\\\\r\\\\nb\"], lines: stdin}))\n",
        "",
    );
    assert_eq!(
        out.trim(),
        r#"[{"Stdout":{"text":"a\r"}},{"Stdout":{"text":"b"}}]"#
    );
}
