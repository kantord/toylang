//! The native runtime is C, and C's memory errors do not show up as a wrong answer: a write one
//! byte past a malloc block goes unnoticed because malloc rounds sizes up. This links a program
//! with the runtime built under AddressSanitizer, so such a write fails the run.
//!
//! The Rust half of the runtime is not instrumented (a sanitizer build of a Rust crate needs a
//! nightly compiler), but its blocks come from libc's malloc, which AddressSanitizer does
//! intercept. So an out-of-range access by the C code to a block the Rust core allocated (a
//! Vec's columns, a record, an Opt box) is caught, and one by the Rust code itself is not; the
//! Rust accessors' own bounds are checked by `cargo test -p toylang-rt`.
//!
//! `link` finds the compiler as `cc` on PATH, so the sanitizer goes in through a `cc` wrapper
//! placed first on the PATH of a `toylang build` child, not through a flag on `link`.

use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

/// Whether the real `cc` can build and run with AddressSanitizer; when it cannot (no libasan
/// installed), the test has nothing to say.
fn cc_supports_asan(dir: &Path) -> bool {
    let probe = dir.join("probe.c");
    std::fs::write(&probe, "int main(void) { return 0; }\n").unwrap();
    Command::new("cc")
        .args(["-fsanitize=address", "-o"])
        .arg(dir.join("probe"))
        .arg(&probe)
        .status()
        .is_ok_and(|s| s.success())
}

/// A `cc` in `bin` that adds -fsanitize=address. It drops its own directory from PATH first, so
/// the `cc` it execs is the real one rather than itself.
fn write_asan_cc(bin: &Path) {
    let wrapper = bin.join("cc");
    let script = format!(
        "#!/bin/sh\nPATH=${{PATH#{}:}}\nexec cc -fsanitize=address -g \"$@\"\n",
        bin.display()
    );
    std::fs::write(&wrapper, script).unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn run_under_asan(program: &str, stdin: &str) -> Option<String> {
    let dir = tempfile::tempdir().unwrap();
    if !cc_supports_asan(dir.path()) {
        return None;
    }
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    write_asan_cc(&bin);
    std::fs::write(dir.path().join("p.toy"), program).unwrap();

    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let build = Command::new(env!("CARGO_BIN_EXE_toylang"))
        .args(["build", "p.toy", "native"])
        .current_dir(dir.path())
        .env("PATH", path)
        .output()
        .unwrap();
    assert!(
        build.status.success(),
        "build failed: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    // Leaks are not the question: the runtime's stance is that nothing frees.
    let mut child = Command::new(dir.path().join("p"))
        .env("ASAN_OPTIONS", "detect_leaks=0")
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
    let run = child.wait_with_output().unwrap();
    assert!(
        run.status.success(),
        "sanitizer stopped the run:\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    Some(String::from_utf8(run.stdout).unwrap())
}

#[test]
fn printing_zero_and_nan_stays_inside_the_allocation() {
    let Some(out) = run_under_asan("[0.0, 0.0 / 0.0]\n", "") else {
        eprintln!("skipped: cc cannot build with -fsanitize=address here");
        return;
    };
    assert_eq!(out, "[0,NaN]\n");
}

/// Every operation that allocates or walks a Vec of records: construction, a column read, a
/// mask (`select`), a reorder of every column, a concatenation, a tail, and the Vec that the JSON
/// reader builds from input. A column one slot too short would show up here.
#[test]
fn vecs_of_records_stay_inside_their_columns() {
    let program = "\
fn rows(db: { users: Vec<{ name: Str, age: Int, ok: Bool }> }) -> Vec<{ name: Str, age: Int, ok: Bool }> =
  reverse(db.users + [{ name: \"lit\", age: 5, ok: true }] | sort_by(.age) | select(.age >= 9));


rows(parse stdin)
";
    let input = r#"{"users": [{"name": "ada", "age": 36, "ok": true}, {"name": "bo", "age": 9, "ok": false}, {"name": "cy", "age": 12, "ok": true}]}"#;
    let Some(out) = run_under_asan(program, input) else {
        eprintln!("skipped: cc cannot build with -fsanitize=address here");
        return;
    };
    assert_eq!(
        out,
        "[{\"name\":\"ada\",\"age\":36,\"ok\":true},{\"name\":\"cy\",\"age\":12,\"ok\":true},{\"name\":\"bo\",\"age\":9,\"ok\":false}]\n"
    );
}

/// Each Vec operation the Rust runtime owns that builds a fresh Vec or gathers a row: the sorts,
/// `max_by`, the reshapes and `tail`/`first`, on scalars, on Strs and on a Vec of records read
/// from input (whose columns the operations must all size and copy together).
#[test]
fn vec_reshapes_stay_inside_their_columns() {
    let program = "\
fn shapes(db: { users: Vec<{ name: Str, age: Int }> }) -> { sorted: Vec<Str>, ints: Vec<Int>, oldest: Opt<{ name: Str, age: Int }>, head: Opt<{ name: Str, age: Int }>, rest: Opt<Vec<{ name: Str, age: Int }>>, grid: Vec<Vec<Int>>, flat: Vec<Int>, some: Bool, every: Bool } =
  {
    sorted: sort([\"pear\", \"apple\", \"fig\"]),
    ints: [3, 1, 2, 1] | sort_by(.),
    oldest: db.users | max_by(.age),
    head: first(db.users),
    rest: tail(db.users),
    grid: transpose([[1, 2, 3], [4, 5, 6]]),
    flat: flatten([[1, 2], [3]]) + reverse([5, 4]),
    some: any([false, true]),
    every: all([true, false])
  };


shapes(parse stdin)
";
    let input = r#"{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": 9}, {"name": "cy", "age": 36}]}"#;
    let Some(out) = run_under_asan(program, input) else {
        eprintln!("skipped: cc cannot build with -fsanitize=address here");
        return;
    };
    assert_eq!(
        out,
        "{\"sorted\":[\"apple\",\"fig\",\"pear\"],\"ints\":[1,1,2,3],\"oldest\":{\"name\":\"ada\",\"age\":36},\"head\":{\"name\":\"ada\",\"age\":36},\"rest\":[{\"name\":\"bo\",\"age\":9},{\"name\":\"cy\",\"age\":36}],\"grid\":[[1,4],[2,5],[3,6]],\"flat\":[1,2,3,4,5],\"some\":true,\"every\":false}\n"
    );
}

/// The rest of the Vec operations: a reduction over a column, indexing through a select mask and
/// straight, a slice, an unwrap one layer down, `range` and `chars`.
#[test]
fn indexing_and_reductions_stay_inside_their_columns() {
    let program = "\
fn probes(db: { users: Vec<{ name: Str, age: Int }> }) -> { total: Int, big: Opt<Int>, last: Opt<{ name: Str, age: Int }>, adult: Opt<{ name: Str, age: Int }>, window: Vec<{ name: Str, age: Int }>, digits: Vec<Int>, letters: Int, firsts: Vec<Int> } =
  {
    total: sum(db.users[].age),
    big: max(db.users[].age),
    last: db.users[-1],
    adult: (db.users | select(.age >= 18))[-1],
    window: db.users[1:],
    digits: collect(range(4)),
    letters: length(chars(\"h\u{e9}\u{1f600}\")),
    firsts: [[7, 8], [9]][][0]!
  };


probes(parse stdin)
";
    let input = r#"{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": 9}, {"name": "cy", "age": 21}]}"#;
    let Some(out) = run_under_asan(program, input) else {
        eprintln!("skipped: cc cannot build with -fsanitize=address here");
        return;
    };
    assert_eq!(
        out,
        "{\"total\":66,\"big\":36,\"last\":{\"name\":\"cy\",\"age\":21},\"adult\":{\"name\":\"cy\",\"age\":21},\"window\":[{\"name\":\"bo\",\"age\":9},{\"name\":\"cy\",\"age\":21}],\"digits\":[0,1,2,3],\"letters\":3,\"firsts\":[7,9]}\n"
    );
}
