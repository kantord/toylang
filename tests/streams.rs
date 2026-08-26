//! Streaming input, in its most minimal cut: `lines`, `collect`, and the rules that keep a
//! single-use stream from ending up somewhere it cannot be honoured.
//!
//! What every backend agrees on when it runs lives in the corpus, as `lines_cat.yaml` and its
//! siblings. This file holds the claims a corpus case cannot express: that a program is
//! refused, and why.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// There is only one real stdin, so a second read is refused at compile time rather than
/// silently handed nothing back, the way Python's own generators are.
#[test]
fn lines_cannot_be_read_twice() {
    insta::assert_snapshot!(err("[collect(lines), collect(lines)]"));
}

/// Forced by jq specifically: raw-input mode, needed for `collect` to read lines rather than
/// JSON, changes what the whole invocation means, so it cannot coexist with `input` being
/// parsed as a JSON document in the same run. Verified against real jq before this rule was
/// added: `jq -Rn '[inputs]'` and ordinary `.`-is-the-document mode are mutually exclusive.
#[test]
fn input_and_lines_cannot_both_be_used() {
    insta::assert_snapshot!(err("fn f(x: Int) -> Int = x\n\nf(input) + (collect(lines) | 0)"));
}

/// A stream has nothing to print: only `collect` turns it into a value. Caught for the
/// program's own result, which has no annotation to check against the way a function's return
/// type does.
#[test]
fn lines_cannot_be_the_programs_result() {
    insta::assert_snapshot!(err("lines"));
}

/// Nothing can get a `Lines` value back out of a Vec once it is in one, and there is only ever
/// one real stdin to hold in the first place.
#[test]
fn lines_cannot_enter_a_vec() {
    insta::assert_snapshot!(err("str(1) | [lines]"));
}

/// Same reasoning as the Vec case, for a record field.
#[test]
fn lines_cannot_enter_a_record() {
    insta::assert_snapshot!(err("{a: lines}"));
}

/// `collect` is an ordinary function, so the bare-application rule reaches it like any other:
/// `collect lines`, with no parens at all, is one more way to spell the acceptance program
/// alongside `collect(lines)`.
#[test]
fn collect_takes_lines_with_or_without_parens() {
    assert!(toylang::compile("collect(lines)").is_ok());
    assert!(toylang::compile("collect lines").is_ok());
}
