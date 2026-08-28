//! Liveness probes for the fused read-one/transform-one/write-one loop (see `tir::fusion`).
//!
//! `tests/corpus.rs` cannot see this feature at all: a fused loop and an eager "read all of
//! stdin, then print everything" implementation produce byte-identical output for any finite
//! input, so output equality alone proves nothing about *when* it arrived. What is different is
//! observable only while the program is still running, which is why this file talks to a live
//! child process instead of waiting for one to exit the way `toylang::run_on` does.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

const PROGRAM: &str = r#"
fn adults(users: Stream<{name: Str, age: Int}>) -> Stream<{name: Str}> =
    users | select(.age >= 18) | map {name: .name}

jsonlines(adults(inputs))
"#;

/// The probe that proves the shape guess actually retired: two stream-signature functions
/// composed. The old `recognize_fusion` unwrapped exactly one call whose argument was `inputs`
/// itself, so `names(keep(inputs))` fell back -- silently -- to materializing all of stdin.
/// Type-driven fusion reads it like any other stream pipeline.
const COMPOSED: &str = r#"
fn keep(users: Stream<{name: Str, age: Int}>) -> Stream<{name: Str, age: Int}> =
    users | select(.age >= 18)

fn names(users: Stream<{name: Str, age: Int}>) -> Stream<{name: Str}> =
    users | map {name: .name}

jsonlines(names(keep(inputs)))
"#;

/// A `lines`-sourced pipeline ending in `jsonlines`: a shape the old recognizer never knew
/// (it demanded an `inputs` base), fused now because the types say stream.
const SHOUT: &str = r#"
fn shout(names: Stream<Str>) -> Stream<Str> =
    names | map(. + "!")

jsonlines(shout(lines))
"#;

const RECORD_IN: &[u8] = b"{\"name\": \"ada\", \"age\": 36}\n";
const RECORD_OUT: &str = r#"{"name":"ada"}"#;

/// Sends one record, then -- without ever sending EOF -- asserts its printed line arrives within
/// a short timeout. An eager implementation is still blocked reading stdin to EOF at this point
/// and never gets here, which is what a timeout here would mean; the fused implementations only
/// need one record to produce one line.
fn assert_streams_first_record(mut child: Child, send: &[u8], expect: &str) {
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));

    stdin.write_all(send).expect("write first record");
    stdin.flush().expect("flush first record");

    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let _ = stdout.read_line(&mut line);
        let _ = tx.send(line);
    });
    let line = rx
        .recv_timeout(Duration::from_secs(5))
        .expect("no output arrived before stdin closed -- this is reading all of stdin first, not streaming");
    assert_eq!(line.trim_end(), expect);

    // Cleanup only, not part of the proof: nothing here is read again, and the child would
    // otherwise sit forever waiting for stdin to close.
    let _ = child.kill();
    let _ = child.wait();
}

fn piped(mut cmd: Command) -> Child {
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the child")
}

/// Lua runs embedded via `mlua`, not as a subprocess `toylang::run_on` spawns and pipes to like
/// every other backend, so there is no child process boundary to test against by calling
/// `emit_lua::emit` directly the way the others call their own `emit_*`. The compiled `toylang`
/// binary itself is that boundary here: it is what actually decides live vs. captured (see
/// `run_lua`'s `feed` handling in `lib.rs`), so this drives it exactly as a real user would.
fn spawn_lua(program: &str) -> Child {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("program.toy");
    std::fs::write(&path, program).expect("write program");

    let mut cmd = Command::new(env!("CARGO_BIN_EXE_toylang"));
    cmd.arg("run").arg(&path).arg("lua");
    // The dir must outlive the child's own read of the source, and the child outlives this
    // function, so leak it for the test's short life rather than racing the cleanup.
    std::mem::forget(dir);
    piped(cmd)
}

fn spawn_native(program: &str) -> Child {
    let program = toylang::compile(program).expect("compiles");
    let dir = tempfile::tempdir().expect("temp dir");
    let exe = dir.path().join("program");
    toylang::link(&program, &exe).expect("compiles and links");
    let cmd = Command::new(&exe);
    std::mem::forget(dir);
    piped(cmd)
}

fn spawn_rust(program: &str) -> Child {
    let program = toylang::compile(program).expect("compiles");
    let source = toylang::emit_rs::emit(&program);
    let dir = tempfile::tempdir().expect("temp dir");
    let exe = dir.path().join("program");
    toylang::link_rust(&source, &exe).expect("compiles with rustc");
    let cmd = Command::new(&exe);
    std::mem::forget(dir);
    piped(cmd)
}

/// The shape `spawn_go`, `spawn_py`, and `spawn_js` share: emit a backend's source to a temp
/// file, then hand it to that backend's own interpreter. `args_before` is `go run`'s subcommand,
/// which comes before the path; `python3` and `node` take the path as their only argument.
fn spawn_interpreted(
    program: &str,
    emit: impl Fn(&toylang::tir::Program) -> String,
    filename: &str,
    interpreter: &str,
    args_before: &[&str],
) -> Child {
    let program = toylang::compile(program).expect("compiles");
    let source = emit(&program);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join(filename);
    std::fs::write(&path, source).expect("write source");

    let mut cmd = Command::new(interpreter);
    cmd.args(args_before).arg(&path);
    std::mem::forget(dir);
    piped(cmd)
}

fn spawn_go(program: &str) -> Child {
    spawn_interpreted(program, toylang::emit_go::emit, "main.go", "go", &["run"])
}

fn spawn_py(program: &str) -> Child {
    spawn_interpreted(
        program,
        toylang::emit_py::emit,
        "program.py",
        "python3",
        &[],
    )
}

fn spawn_js(program: &str) -> Child {
    spawn_interpreted(program, toylang::emit_js::emit, "program.js", "node", &[])
}

fn spawn_jq(program: &str) -> Child {
    let program = toylang::compile(program).expect("compiles");
    let source = toylang::emit_jq::emit(&program);

    let mut cmd = Command::new("jq");
    cmd.arg("--unbuffered").arg("-c").arg("-n").arg("-r");
    if program.uses_lines {
        cmd.arg("-R");
    }
    cmd.arg(&source);
    piped(cmd)
}

#[test]
fn lua_streams() {
    assert_streams_first_record(spawn_lua(PROGRAM), RECORD_IN, RECORD_OUT);
}

#[test]
fn native_streams() {
    assert_streams_first_record(spawn_native(PROGRAM), RECORD_IN, RECORD_OUT);
}

#[test]
fn rust_streams() {
    assert_streams_first_record(spawn_rust(PROGRAM), RECORD_IN, RECORD_OUT);
}

#[test]
fn go_streams() {
    assert_streams_first_record(spawn_go(PROGRAM), RECORD_IN, RECORD_OUT);
}

#[test]
fn py_streams() {
    assert_streams_first_record(spawn_py(PROGRAM), RECORD_IN, RECORD_OUT);
}

#[test]
fn js_streams() {
    assert_streams_first_record(spawn_js(PROGRAM), RECORD_IN, RECORD_OUT);
}

#[test]
fn jq_streams() {
    assert_streams_first_record(spawn_jq(PROGRAM), RECORD_IN, RECORD_OUT);
}

#[test]
fn lua_streams_composed_functions() {
    assert_streams_first_record(spawn_lua(COMPOSED), RECORD_IN, RECORD_OUT);
}

#[test]
fn native_streams_composed_functions() {
    assert_streams_first_record(spawn_native(COMPOSED), RECORD_IN, RECORD_OUT);
}

#[test]
fn rust_streams_composed_functions() {
    assert_streams_first_record(spawn_rust(COMPOSED), RECORD_IN, RECORD_OUT);
}

#[test]
fn go_streams_composed_functions() {
    assert_streams_first_record(spawn_go(COMPOSED), RECORD_IN, RECORD_OUT);
}

#[test]
fn py_streams_composed_functions() {
    assert_streams_first_record(spawn_py(COMPOSED), RECORD_IN, RECORD_OUT);
}

#[test]
fn js_streams_composed_functions() {
    assert_streams_first_record(spawn_js(COMPOSED), RECORD_IN, RECORD_OUT);
}

#[test]
fn jq_streams_composed_functions() {
    assert_streams_first_record(spawn_jq(COMPOSED), RECORD_IN, RECORD_OUT);
}

/// One interpreted and one compiled backend for the `lines` source; the output equality of the
/// shape on all seven is `jsonlines_of_lines.yaml`'s job.
#[test]
fn py_streams_lines() {
    assert_streams_first_record(spawn_py(SHOUT), b"ada\n", r#""ada!""#);
}

#[test]
fn native_streams_lines() {
    assert_streams_first_record(spawn_native(SHOUT), b"ada\n", r#""ada!""#);
}
