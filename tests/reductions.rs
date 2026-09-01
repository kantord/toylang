//! What the checker refuses about `sum` and `max` (kantord/toylang#140).

/// `sum` is defined only for the two integer element types, so a scalar argument is refused
/// with the restricted set named, the way `sort` names its own.
#[test]
fn sum_takes_a_vec() {
    insta::assert_snapshot!(
        toylang::compile(r#"sum(1)"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

/// Neither Str nor Char reduces (the ruling cut them on the same no-caller grounds it cut min
/// and product), so a Vec of either is refused rather than silently ordered.
#[test]
fn sum_takes_an_int_element() {
    insta::assert_snapshot!(
        toylang::compile(r#"sum(["a"])"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

#[test]
fn max_takes_a_vec() {
    insta::assert_snapshot!(
        toylang::compile(r#"max(1)"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

/// A record has no total order, so a Vec of records cannot be reduced to a maximum.
#[test]
fn max_takes_an_int_element() {
    insta::assert_snapshot!(
        toylang::compile(r#"max([{n: 1}])"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

/// A builtin is a reserved name: a program that defines `sum` and means something else by it
/// is refused the same way `str` is, rather than silently shadowed.
#[test]
fn a_builtin_cannot_be_redefined() {
    insta::assert_snapshot!(
        toylang::compile("fn sum(x: Vec<Int>) -> Int = x[0]!\n\nsum([1])")
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}
