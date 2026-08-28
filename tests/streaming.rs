//! Liveness probes for the backends that fuse `jsonlines(f(inputs))` into a read-one/
//! transform-one/write-one loop (see `tir::recognize_fusion`).
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

/// Sends one record, then -- without ever sending EOF -- asserts its printed line arrives within
/// a short timeout. An eager implementation is still blocked reading stdin to EOF at this point
/// and never gets here, which is what a timeout here would mean; the fused implementations only
/// need one record to produce one line.
fn assert_streams_first_record(mut child: Child) {
    let mut stdin = child.stdin.take().expect("piped stdin");
    let mut stdout = BufReader::new(child.stdout.take().expect("piped stdout"));

    stdin.write_all(b"{\"name\": \"ada\", \"age\": 36}\n").expect("write first record");
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
    assert_eq!(line.trim_end(), r#"{"name":"ada"}"#);

    // Cleanup only, not part of the proof: nothing here is read again, and the child would
    // otherwise sit forever waiting for stdin to close.
    let _ = child.kill();
    let _ = child.wait();
}

/// Lua runs embedded via `mlua`, not as a subprocess `toylang::run_on` spawns and pipes to like
/// every other backend, so there is no child process boundary to test against by calling
/// `emit_lua::emit` directly the way the others call their own `emit_*`. The compiled `toylang`
/// binary itself is that boundary here: it is what actually decides live vs. captured (see
/// `run_lua`'s `feed` handling in `lib.rs`), so this drives it exactly as a real user would.
#[test]
fn lua_streams() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("program.toy");
    std::fs::write(&path, PROGRAM).expect("write program");

    let child = Command::new(env!("CARGO_BIN_EXE_toylang"))
        .arg("run")
        .arg(&path)
        .arg("lua")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run the toylang binary");
    assert_streams_first_record(child);
}

#[test]
fn native_streams() {
    let program = toylang::compile(PROGRAM).expect("compiles");
    let dir = tempfile::tempdir().expect("temp dir");
    let exe = dir.path().join("program");
    toylang::link(&program, &exe).expect("compiles and links");

    let child = Command::new(&exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run the compiled binary");
    assert_streams_first_record(child);
}

#[test]
fn rust_streams() {
    let program = toylang::compile(PROGRAM).expect("compiles");
    let source = toylang::emit_rs::emit(&program);
    let dir = tempfile::tempdir().expect("temp dir");
    let exe = dir.path().join("program");
    toylang::link_rust(&source, &exe).expect("compiles with rustc");

    let child = Command::new(&exe)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run the compiled binary");
    assert_streams_first_record(child);
}

#[test]
fn go_streams() {
    let program = toylang::compile(PROGRAM).expect("compiles");
    let source = toylang::emit_go::emit(&program);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("main.go");
    std::fs::write(&path, source).expect("write source");

    let child = Command::new("go")
        .arg("run")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run go");
    assert_streams_first_record(child);
}

#[test]
fn py_streams() {
    let program = toylang::compile(PROGRAM).expect("compiles");
    let source = toylang::emit_py::emit(&program);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("program.py");
    std::fs::write(&path, source).expect("write source");

    let child = Command::new("python3")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run python3");
    assert_streams_first_record(child);
}

#[test]
fn js_streams() {
    let program = toylang::compile(PROGRAM).expect("compiles");
    let source = toylang::emit_js::emit(&program);
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("program.js");
    std::fs::write(&path, source).expect("write source");

    let child = Command::new("node")
        .arg(&path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run node");
    assert_streams_first_record(child);
}

#[test]
fn jq_streams() {
    let program = toylang::compile(PROGRAM).expect("compiles");
    let source = toylang::emit_jq::emit(&program);

    let child = Command::new("jq")
        .arg("--unbuffered")
        .arg("-c")
        .arg("-n")
        .arg("-r")
        .arg(&source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("run jq");
    assert_streams_first_record(child);
}
