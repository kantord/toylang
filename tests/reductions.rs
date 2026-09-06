//! What the checker refuses about `sum`, `max`, and the projection-ordered `sort_by`/`max_by`
//! (kantord/toylang#140, gh:177).

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

/// The projection-ordered pair (gh:177) type-checks on a Vec of records, ordering by the
/// projected scalar the same way `map` reads it.
#[test]
fn sort_by_and_max_by_compile() {
    assert!(toylang::compile("[{n: 2}, {n: 1}] | sort_by(.n)").is_ok());
    assert!(toylang::compile("[{n: 2}, {n: 1}] | max_by(.n)").is_ok());
}

/// Blocking like `sort` and `max`, both take a Vec only, so a scalar subject is refused.
#[test]
fn sort_by_takes_a_vec() {
    insta::assert_snapshot!(
        toylang::compile(r#"1 | sort_by(.)"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

#[test]
fn max_by_takes_a_vec() {
    insta::assert_snapshot!(
        toylang::compile(r#"1 | max_by(.)"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

/// Ordering is by the projected key, so the projection is held to the same natively-ordered
/// scalars `sort` takes; a projection that is itself a record or a Vec is refused.
#[test]
fn sort_by_projection_must_be_orderable() {
    insta::assert_snapshot!(
        toylang::compile(r#"[{n: 1}] | sort_by(.)"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

#[test]
fn max_by_projection_must_be_orderable() {
    insta::assert_snapshot!(
        toylang::compile(r#"[{n: 1}] | max_by(.)"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}
