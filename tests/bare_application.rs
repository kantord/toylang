//! Bare (parenless) application: `f x` reads as `f(x)`, and it is the default calling style,
//! with `f(x)` as the explicit disambiguator. One rule confines it: an argument -- bare or
//! delimited -- must start on the same line as its function, which is what keeps a
//! definition's trailing identifier from swallowing the program body that follows it.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn matches_the_parenthesized_form() {
    assert_eq!(
        toylang::run("str 5").unwrap(),
        toylang::run("str(5)").unwrap()
    );
}

/// Right-recursive, so `f g x` is `f(g(x))` rather than needing `f(g(x))` spelled with parens.
#[test]
fn chains_right_to_left() {
    let chained = "fn inc(n: Int) -> Int = n + 1\n\nstr inc 5";
    let parenthesized = "fn inc(n: Int) -> Int = n + 1\n\nstr(inc(5))";
    assert_eq!(
        toylang::run(chained).unwrap(),
        toylang::run(parenthesized).unwrap()
    );
}

/// A bare call is an ordinary atom, so it composes inside larger expressions: the argument is
/// a postfix chain, and an infix operator after it belongs to the enclosing expression, making
/// `1 + inc 2 + inc 3` read `(1 + inc(2)) + inc(3)`.
#[test]
fn composes_with_operators() {
    let src = "fn inc(n: Int) -> Int = n + 1\n\n1 + inc 2 + inc 3";
    assert_eq!(toylang::run(src).unwrap(), "8\n");
}

/// The definition-body suspension is gone: `= extent v` is a call, and the same-line rule is
/// what keeps the definition boundary safe instead.
#[test]
fn a_definition_body_may_end_in_a_bare_call() {
    let src = "fn size(v: Vec<Int>) -> Int = extent v\n\nsize([1, 2, 3])";
    assert_eq!(toylang::run(src).unwrap(), "3\n");
}

/// An argument on the line below its function is not an argument -- that reading is what let a
/// definition's body swallow the program's -- and the leftover token's error says how to spell
/// the call across lines.
#[test]
fn a_cross_line_argument_is_rejected_naming_the_parens_spelling() {
    insta::assert_snapshot!(err("fn inc(n: Int) -> Int = n + 1\n\ninc\n1"));
}

/// `-` is already subtraction, so `f -1` stays `f - 1` rather than `f` applied to `-1` -- the
/// same resolution Haskell gives the identical clash. `f` is then a function where a value is
/// needed, and the error says that instead of claiming the name is undefined.
#[test]
fn a_trailing_minus_is_subtraction_not_negation() {
    insta::assert_snapshot!(err("fn f(n: Int) -> Int = n\n\nf -1"));
}

/// Projection binds tighter than bare application everywhere, so `map .n` is a field access on
/// `map` -- a function where a value is needed -- and passing a projection as an argument takes
/// the parens spelling, `map(.n)`.
#[test]
fn projection_wins_over_bare_application() {
    insta::assert_snapshot!(err("[{n: 1}] | map .n"));
}

/// `select` and `map` carry no dedicated syntax any more: they are ordinary calls, checked by
/// name inside `Call`, and so are reserved the same way every other builtin is.
#[test]
fn select_and_map_cannot_be_redefined() {
    insta::assert_snapshot!(err("fn select(x: Int) -> Int = x\n\n1"));
    insta::assert_snapshot!(err("fn map(x: Int) -> Int = x\n\n1"));
}

/// A definition's body and whatever follows it -- another `fn`, or the file's own body -- sit
/// adjacent with no token between them, the one place two `expr` parses meet without a delimiter
/// to anchor on. The same-line rule is what keeps a trailing bare identifier in a body from
/// reaching across that boundary and swallowing the next definition or the program's body.
#[test]
fn a_function_body_does_not_swallow_what_follows_it() {
    assert_eq!(
        toylang::run("fn f(x: Int) -> Int = x\n\nf(1)").unwrap(),
        "1\n"
    );
}

/// The rule covers the parenthesized argument too -- a reach across the boundary the old
/// suspension flag never gated, since it only ever switched the bare form off.
#[test]
fn a_parenthesized_program_body_is_not_swallowed() {
    assert_eq!(
        toylang::run("fn f(x: Int) -> Int = x\n\n(1 + 2)").unwrap(),
        "3\n"
    );
}
