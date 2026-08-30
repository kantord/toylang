//! `toylang fmt` walking a project: what it lists, what it writes, and what it exits with.
//!
//! Driven through the real binary rather than `fmt_tree::run` directly, because the exit code is
//! half of what this feature promises -- a check mode that reported correctly and exited zero
//! would gate nothing -- and an exit code only exists once there is a process to have one.

use std::path::Path;
use std::process::{Command, Output};

/// One line off canonical: `fmt` spaces its binary operators.
const CROOKED: &str = "fn double(n: Int) -> Int = n*2\ndouble(21)\n";
const CANONICAL: &str = "fn double(n: Int) -> Int = n * 2\n\ndouble(21)\n";

#[test]
fn check_lists_what_is_not_formatted_and_touches_nothing() {
    let dir = tempfile::tempdir().expect("a temp dir");
    write(dir.path(), "crooked.toy", CROOKED);
    write(dir.path(), "straight.toy", CANONICAL);

    let out = fmt(dir.path(), &[]);

    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(stdout(&out), "crooked.toy\n");
    assert_eq!(
        read(dir.path(), "crooked.toy"),
        CROOKED,
        "check wrote a file"
    );
}

#[test]
fn write_rewrites_what_check_would_have_listed_and_still_exits_nonzero() {
    let dir = tempfile::tempdir().expect("a temp dir");
    write(dir.path(), "crooked.toy", CROOKED);

    let first = fmt(dir.path(), &["--write"]);
    assert_eq!(first.status.code(), Some(1), "{}", stderr(&first));
    assert_eq!(stdout(&first), "crooked.toy\n");
    assert_eq!(read(dir.path(), "crooked.toy"), CANONICAL);

    // The second run is the whole point of the exit contract: nonzero means "this run changed
    // something", not "this tree is unformattable", so it has to clear once the tree is clean.
    let second = fmt(dir.path(), &["--write"]);
    assert_eq!(second.status.code(), Some(0), "{}", stderr(&second));
    assert_eq!(stdout(&second), "");
}

/// The walk descends, in a stable order, and stays inside the project's own source: not `.git`
/// or any other dot-folder, and not a file that merely contains toylang, like a markdown page.
#[test]
fn the_walk_is_recursive_sorted_and_skips_hidden_folders() {
    let dir = tempfile::tempdir().expect("a temp dir");
    write(dir.path(), "b.toy", CROOKED);
    std::fs::create_dir_all(dir.path().join("nested/deeper")).expect("a nested dir");
    write(dir.path(), "nested/a.toy", CROOKED);
    write(dir.path(), "nested/deeper/a.toy", CROOKED);
    std::fs::create_dir(dir.path().join(".hidden")).expect("a hidden dir");
    write(dir.path(), ".hidden/a.toy", CROOKED);
    write(dir.path(), "page.md", CROOKED);

    let out = fmt(dir.path(), &[]);

    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "b.toy\nnested/a.toy\nnested/deeper/a.toy\n",
        "expected every .toy file below the folder, sorted, and nothing else"
    );
}

/// A file the parser rejects is not "already formatted": it is a file the formatter could not
/// read, which the run has to say out loud rather than pass over in silence. `--write` leaves it
/// exactly as it found it -- there is no canonical form to write.
#[test]
fn a_file_the_parser_rejects_is_reported_and_left_alone() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let broken = "fn double(n: Int) -> Int =\n";
    write(dir.path(), "broken.toy", broken);

    let out = fmt(dir.path(), &["--write"]);

    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        stdout(&out),
        "",
        "a file that will not parse was not rewritten"
    );
    assert!(
        stderr(&out).starts_with("toylang: broken.toy: "),
        "expected the path and the parse error, got: {}",
        stderr(&out)
    );
    assert_eq!(read(dir.path(), "broken.toy"), broken);
}

/// The filter form, unchanged by any of the above: one named file, formatted to stdout, and the
/// file on disk left where it was.
#[test]
fn naming_a_file_still_formats_it_to_stdout() {
    let dir = tempfile::tempdir().expect("a temp dir");
    write(dir.path(), "crooked.toy", CROOKED);

    let out = fmt(dir.path(), &["crooked.toy"]);

    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out), CANONICAL);
    assert_eq!(read(dir.path(), "crooked.toy"), CROOKED);
}

fn fmt(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_toylang"))
        .arg("fmt")
        .args(args)
        .current_dir(dir)
        .output()
        .expect("the toylang binary runs")
}

fn write(dir: &Path, name: &str, contents: &str) {
    std::fs::write(dir.join(name), contents).unwrap_or_else(|e| panic!("writing {name}: {e}"));
}

fn read(dir: &Path, name: &str) -> String {
    std::fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("reading {name}: {e}"))
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("utf-8 stdout")
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("utf-8 stderr")
}
