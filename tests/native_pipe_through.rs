//! What a built native program does around `pipe_through` that the shared tests in
//! `tests/pipe_through.rs` cannot see, because it is specific to the native runtime: the
//! failure messages, and how SIGPIPE is handled. The generated `main` does not go through Rust's
//! `lang_start`, so SIGPIPE keeps its default action in the program itself; these run the built
//! binary directly.

use std::io::Write;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, Stdio};

struct Built {
    _dir: tempfile::TempDir,
    exe: std::path::PathBuf,
}

fn build(program: &str) -> Built {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("p.toy"), program).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_toylang"))
        .args(["build", "p.toy", "native"])
        .current_dir(dir.path())
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let exe = dir.path().join("p");
    Built { _dir: dir, exe }
}

/// Runs the program with `stdin`, from a shell that ignored SIGPIPE first when `ignoring_sigpipe`
/// (an ignored signal survives `exec`, as it does when a CI runner or a Python `subprocess` call
/// starts the program).
fn run(built: &Built, stdin: &str, ignoring_sigpipe: bool) -> std::process::Output {
    let mut command = if ignoring_sigpipe {
        let mut c = Command::new("sh");
        c.args(["-c", "trap '' PIPE; exec \"$0\""]).arg(&built.exe);
        c
    } else {
        Command::new(&built.exe)
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn a_command_that_cannot_be_spawned_names_it_and_exits_1() {
    let built = build(
        "collect(pipe_through({cmd: \"toylang-no-such-command\", args: [], lines: stdin}))\n",
    );
    let out = run(&built, "", false);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "toylang: cannot spawn subprocess `toylang-no-such-command`: No such file or directory\n"
    );
}

#[test]
fn output_that_is_not_utf8_is_a_failure_with_a_message() {
    let built = build(
        "collect(pipe_through({cmd: \"printf\", args: [\"ok\\\\n\\\\377\"], lines: stdin}))\n",
    );
    let out = run(&built, "", false);
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(out.stdout, b"");
    assert_eq!(
        String::from_utf8_lossy(&out.stderr),
        "toylang: input: subprocess output is not valid UTF-8 at pipe_through\n"
    );
}

/// A child that reads nothing and exits at once, with a lot of input still unsent. The write
/// that finds the pipe closed raises SIGPIPE, and the default action would end the program.
#[test]
fn writing_to_a_child_that_already_exited_does_not_kill_the_program() {
    let built =
        build("collect(pipe_through({cmd: \"true\", args: [], lines: stdin})) | length(.)\n");
    let input: String = (0..200_000).map(|i| format!("{i}\n")).collect();
    for ignoring in [false, true] {
        let out = run(&built, &input, ignoring);
        assert_eq!(out.status.code(), Some(0), "ignoring SIGPIPE: {ignoring}");
        assert_eq!(out.stdout, b"0\n");
    }
}

/// `yes | head` inside the child ends `yes` with SIGPIPE, silently, only if the child got the
/// default action back. Under an ignored one `yes` reports `Broken pipe` on stderr. Started from
/// a shell that ignores SIGPIPE, the program itself has it ignored, so this is what resetting it
/// for the child is for.
#[test]
fn the_child_gets_the_default_sigpipe_action_even_when_the_program_was_started_ignoring_it() {
    let built = build(
        "collect(pipe_through({cmd: \"sh\", args: [\"-c\", \"yes | head -n 1\"], lines: stdin}))\n",
    );
    for ignoring in [false, true] {
        let out = run(&built, "", ignoring);
        assert_eq!(out.status.code(), Some(0), "ignoring SIGPIPE: {ignoring}");
        assert_eq!(
            String::from_utf8_lossy(&out.stdout),
            "[{\"Stdout\":{\"text\":\"y\"}}]\n"
        );
    }
}

/// The program's own stdout closed early ends it with SIGPIPE, as it does any Unix filter, and
/// running `pipe_through` first leaves that as it was: nothing restores or forgets it. The
/// reader closes its end before the program has anything to print (the program is held up on
/// its stdin), so the first write fails.
#[test]
fn a_closed_stdout_still_ends_the_program_after_a_pipe_through() {
    let built = build("collect(pipe_through({cmd: \"cat\", args: [], lines: stdin}))\n");
    let mut child = Command::new(&built.exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    drop(child.stdout.take());
    child.stdin.take().unwrap().write_all(b"a\nb\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert_eq!(out.status.signal(), Some(13), "{:?}", out.status);
}
