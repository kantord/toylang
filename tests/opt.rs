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
    assert!(toylang::compile("fn f(x: Opt<Int>) -> Int = x | 0\n\nf([1][0])").is_ok());
}

/// `Opt` nests the same way `Vec` does: nothing in the grammar or the checker singles out one
/// level, so `Opt<Opt<T>>` is exactly as legal as `Opt<T>`, matching what collapsing a `Vec<Opt<T>>`
/// already produced before `Opt` had a spelling.
#[test]
fn opt_can_hold_an_opt() {
    assert!(toylang::compile("fn f(x: Opt<Opt<Int>>) -> Int = x | 0\n\n1").is_ok());
}

#[test]
fn a_vec_can_hold_an_opt() {
    assert!(toylang::compile("fn f(x: Vec<Opt<Int>>) -> Int = x | 0\n\n1").is_ok());
}

#[test]
fn an_opt_can_hold_a_vec() {
    assert!(toylang::compile("fn f(x: Opt<Vec<Int>>) -> Int = x | 0\n\n1").is_ok());
}

#[test]
fn a_record_field_can_be_an_opt() {
    assert!(toylang::compile("fn f(r: {a: Opt<Int>}) -> Int = r | 0\n\n1").is_ok());
}

#[test]
fn an_enum_payload_can_be_an_opt() {
    assert!(toylang::compile("enum E { v(Opt<Int>) }\n\n1").is_ok());
}

/// `Opt` is the prelude's declaration now, not a reserved name, so redeclaring it collides
/// the way any duplicate type does.
#[test]
fn opt_cannot_be_redeclared() {
    insta::assert_snapshot!(err("type Opt = Int\n\n1"));
}

/// The stream containment ban holds through the generic path `Opt` resolves by now: a stream
/// is not a value, so it cannot be a type argument any more than it could sit in a Vec.
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

/// Deliberately parameterized, not built: how `some`/`none` arms compose is what the pending
/// matcher-totality round decides (plans/opt-as-enum.md, "Open points, owned elsewhere").
#[test]
fn matching_an_opt_by_variant_is_not_yet_decided() {
    insta::assert_snapshot!(err("[1, 2][0] | some -> 1 or none -> 0"));
}

/// Absence has no ratified wire form: serialization emits null going out, and whether null
/// coming in reads as `none` is codec design nobody has done, so an Opt anywhere in an input
/// type is refused rather than guessed.
#[test]
fn input_cannot_be_opt_typed() {
    insta::assert_snapshot!(err("fn f(x: Opt<Int>) -> Int = x!\n\nf(input)"));
}

#[test]
fn inputs_cannot_carry_an_opt_element() {
    insta::assert_snapshot!(err(
        "fn f(v: Vec<Opt<Int>>) -> Int = extent(v)\n\nf(collect(inputs))"
    ));
}

/// The generic-unit rule (tests/generics.rs) worded at the type everyone will actually hit.
#[test]
fn none_alone_cannot_be_synthesised() {
    insta::assert_snapshot!(err("none"));
}
