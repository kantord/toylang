//! The prelude's `enum Bool { True, False }` (the bool-literal-decide ruling): what the checker
//! refuses. The corpus carries the positive behaviour -- the constructors, the match, printing
//! and input -- on every backend; these pin the messages, which no backend runs.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// The closed-world rule an enum match has, applied to Bool: the missing matcher is named.
#[test]
fn a_match_over_bool_missing_an_arm() {
    insta::assert_snapshot!(err("true | True -> \"yes\""));
}

/// Both matchers cover the type, so a third arm is dead, the same way it is after every variant
/// of a declared enum.
#[test]
fn an_arm_after_both_matchers() {
    insta::assert_snapshot!(err("true | True -> 1 or False -> 0 or any() -> 2"));
}

/// `True` is the matcher; the value is built with the lowercase constructor, as for any enum.
#[test]
fn a_matcher_used_as_a_value() {
    insta::assert_snapshot!(err("True"));
}

/// `Bool` is the prelude's declaration and a built-in name at once; a program cannot declare a
/// second one under either reading.
#[test]
fn a_program_declaring_its_own_bool() {
    insta::assert_snapshot!(err("enum Bool { Yes, No }\n\ntrue"));
}

/// The bare-until-ambiguous rule reaches `true` too: a program enum that declares a `True`
/// variant makes the bare constructor ambiguous, and the fix is the qualified spelling.
#[test]
fn a_bare_constructor_shared_with_a_program_enum() {
    insta::assert_snapshot!(err("enum Toggle { True, False }\n\ntrue"));
}
