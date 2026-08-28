//! `jsonlines`, the sink: legal only as the program's outermost expression, taking a `Vec<T>`
//! or a `Stream<T>` and printing one JSON value per line, with no result type at all -- nothing
//! remains that could observe one.
//!
//! What every backend agrees on when it runs lives in the corpus. This file holds the claims a
//! corpus case cannot express: a bad argument is refused, the name is reserved, and the sink
//! cannot be nested.

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

/// A sink has no result type, so there is nowhere a nested `jsonlines` could put one: it is
/// legal only as the program's outermost expression.
#[test]
fn jsonlines_cannot_be_nested() {
    insta::assert_snapshot!(err("jsonlines([jsonlines([1])])"));
}
