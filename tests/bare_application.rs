//! Bare (parenless) application: `f x` reads as `f(x)`. Legal only where an expression begins
//! fresh -- a pipe stage, a function body, inside `(...)`/`[...]`/`{...}` -- and never as an
//! operand, so it cannot surface partway through a larger expression. `select` and `map` are not
//! special syntax; they are ordinary names reached through this same rule.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn matches_the_parenthesized_form() {
    assert_eq!(toylang::run("str 5").unwrap(), toylang::run("str(5)").unwrap());
}

/// Right-recursive, so `f g x` is `f(g(x))` rather than needing `f(g(x))` spelled with parens.
#[test]
fn chains_right_to_left() {
    let chained = "fn inc(n: Int) -> Int = n + 1\n\nstr inc 5";
    let parenthesized = "fn inc(n: Int) -> Int = n + 1\n\nstr(inc(5))";
    assert_eq!(toylang::run(chained).unwrap(), toylang::run(parenthesized).unwrap());
}

/// `operand` and everything it calls (`unary`, `postfix`, `atom`) never look for a bare call, so
/// one cannot appear as the right side of `+`: `str` is read as a plain variable reference, which
/// leaves `2` with nowhere to go.
#[test]
fn cannot_be_an_operand() {
    insta::assert_snapshot!(err("1 + str 2"));
}

/// The argument stops at the first infix operator rather than being silently swallowed into it,
/// so `str 5 + 1` is a parse error (the caller has nowhere to put the trailing `+ 1`) instead of
/// a silent `(str 5) + 1`.
#[test]
fn cannot_be_followed_by_an_operator() {
    insta::assert_snapshot!(err("str 5 + 1"));
}

/// `-` is already subtraction, so `f -1` stays `f - 1` rather than `f` applied to `-1` -- the
/// same resolution Haskell gives the identical clash. `f` is then read as a plain variable, which
/// is not one, so the error is the ordinary unbound-name error rather than anything about `-`.
#[test]
fn a_trailing_minus_is_subtraction_not_negation() {
    insta::assert_snapshot!(err("fn f(n: Int) -> Int = n\n\nf -1"));
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
/// to anchor on. A trailing bare identifier in a body must not reach across that boundary and
/// swallow the next definition.
#[test]
fn a_function_body_does_not_swallow_what_follows_it() {
    assert_eq!(toylang::run("fn f(x: Int) -> Int = x\n\nf(1)").unwrap(), "1\n");
}
