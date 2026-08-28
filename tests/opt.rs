//! `Opt<T>` in the type grammar: a function can now declare and return one instead of being
//! forced to unwrap. The behavioral cases (what an `Opt` prints, how `!` consumes one) already
//! live in tests/corpus and tests/streams.rs; this file is about the annotation itself.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// The motivating case from the issue: a function that collapses a dimension can now say so in
/// its own return type instead of the caller only finding out from the value.
#[test]
fn a_function_can_return_an_opt() {
    assert!(toylang::compile("fn head(v: Vec<Int>) -> Opt<Int> = v[0]\n\nhead([1, 2, 3])").is_ok());
}

#[test]
fn a_function_can_take_an_opt() {
    assert!(toylang::compile("fn f(x: Opt<Int>) -> Int = 0\n\nf([1][0])").is_ok());
}

/// `Opt` nests the same way `Vec` does: nothing in the grammar or the checker singles out one
/// level, so `Opt<Opt<T>>` is exactly as legal as `Opt<T>`, matching what collapsing a `Vec<Opt<T>>`
/// already produced before `Opt` had a spelling.
#[test]
fn opt_can_hold_an_opt() {
    assert!(toylang::compile("fn f(x: Opt<Opt<Int>>) -> Int = 0\n\n1").is_ok());
}

#[test]
fn a_vec_can_hold_an_opt() {
    assert!(toylang::compile("fn f(x: Vec<Opt<Int>>) -> Int = 0\n\n1").is_ok());
}

#[test]
fn an_opt_can_hold_a_vec() {
    assert!(toylang::compile("fn f(x: Opt<Vec<Int>>) -> Int = 0\n\n1").is_ok());
}

#[test]
fn a_record_field_can_be_an_opt() {
    assert!(toylang::compile("fn f(r: {a: Opt<Int>}) -> Int = 0\n\n1").is_ok());
}

#[test]
fn an_enum_payload_can_be_an_opt() {
    assert!(toylang::compile("enum E { v(Opt<Int>) }\n\n1").is_ok());
}

/// `Opt` joins `Vec` and `Stream` as a reserved constructor, so it still cannot be redeclared as
/// an alias or an enum name.
#[test]
fn opt_cannot_be_redeclared() {
    insta::assert_snapshot!(err("type Opt = Int\n\n1"));
}

/// The stream containment ban applies to `Opt` exactly as it does to `Vec` and a record field:
/// a stream is not a value, so nothing can describe one as stored.
mod containment {
    use super::err;

    #[test]
    fn a_signature_cannot_put_a_stream_in_an_opt() {
        insta::assert_snapshot!(err("fn f(x: Opt<Stream<Str>>) -> Int = 0\n\n1"));
    }

    #[test]
    fn a_record_field_cannot_put_a_stream_in_an_opt() {
        insta::assert_snapshot!(err("fn f(r: {a: Opt<Stream<Str>>}) -> Int = 0\n\n1"));
    }

    #[test]
    fn an_enum_payload_cannot_put_a_stream_in_an_opt() {
        insta::assert_snapshot!(err("enum E { v(Opt<Stream<Str>>) }\n\n1"));
    }
}
