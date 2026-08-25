//! `jsonlines`, the one builtin polymorphic over its element type: `Vec<T> -> Str` for any
//! printable `T`, reusing the same per-type encoding the top-level printer uses but joined by
//! newline instead of wrapped in `[...]`.
//!
//! What every backend agrees on when it runs lives in the corpus. This file holds the two
//! claims a corpus case cannot express: a bad argument is refused, and the name is reserved.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// Polymorphic, so it is not in the fixed builtin table `fn`-redefinition normally checks
/// against; it needs its own guard for the same reason every other builtin has one.
#[test]
fn jsonlines_cannot_be_redefined() {
    insta::assert_snapshot!(err("fn jsonlines(x: Int) -> Int = x\n\njsonlines(1)"));
}

#[test]
fn jsonlines_needs_a_vec() {
    insta::assert_snapshot!(err(r#"jsonlines("not a vec")"#));
}
