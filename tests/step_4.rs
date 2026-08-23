fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn filter_a_vec() {
    insta::assert_snapshot!(toylang::run("[1, 2, 3] | select(. >= 2)").unwrap());
}

/// `[]` says what happens to a dimension, so with no access after it there is nothing for it to
/// say. This replaces a test that asserted `[]` was the identity, which is the behaviour the
/// spec rule removed.
#[test]
fn a_spec_with_nothing_to_spec() {
    insta::assert_snapshot!(err("[1, 2, 3][] | select(. >= 2)"));
}

/// Every dimension needs a spec, so reaching a component through one without saying so fails.
#[test]
fn field_access_through_an_unspecced_dimension() {
    insta::assert_snapshot!(err(r#"
fn f(db: {users: Vec<{name: Str}>}) -> Vec<Str> = db.users.name
f(input)
"#));
}

#[test]
fn filter_strings() {
    insta::assert_snapshot!(toylang::run(r#"["ada", "bo"] | select(. == "ada")"#).unwrap());
}

#[test]
fn pipe_rebinds_the_subject() {
    insta::assert_snapshot!(toylang::run("[1, 2, 3] | select(. >= 2) | select(. >= 3)").unwrap());
}

#[test]
fn a_vec_survives_a_function() {
    let src = r#"
fn big(xs: Vec<Int>) -> Vec<Int> = xs | select(. >= 2)

big([1, 2, 3])
"#;
    insta::assert_snapshot!(toylang::run(src).unwrap());
}

#[test]
fn emitted_lua() {
    let p = toylang::compile("[1, 2, 3] | select(. >= 2)").unwrap();
    let (lua, ty) = (toylang::emit_lua::emit(&p), p.body.ty.clone());
    insta::assert_snapshot!(format!("-- : {ty}\n{lua}"));
}

#[test]
fn select_needs_a_bool() {
    insta::assert_snapshot!(err("[1, 2, 3] | select(.)"));
}

/// Q2 is open, so an operator over a Vec is rejected rather than silently given broadcast or
/// zip semantics.
#[test]
fn an_operator_does_not_apply_to_a_vec() {
    insta::assert_snapshot!(err(r#"[1, 2] + "a""#));
}

#[test]
fn subject_outside_a_pipeline() {
    insta::assert_snapshot!(err("."));
}

#[test]
fn select_without_a_subject() {
    insta::assert_snapshot!(err("select(. >= 2)"));
}

/// A spec needs a dimension to spec. Written with an access after it, so that it reaches the
/// dimension check rather than stopping at the rule that a spec must be followed by one.
#[test]
fn spec_on_something_with_no_dimension() {
    insta::assert_snapshot!(err("1[].foo"));
}

#[test]
fn empty_vec_literal() {
    insta::assert_snapshot!(err("[]"));
}

#[test]
fn heterogeneous_vec_literal() {
    insta::assert_snapshot!(err(r#"[1, "a"]"#));
}

#[test]
fn comparison_across_types() {
    insta::assert_snapshot!(err(r#"[1, 2] | select(. >= "a")"#));
}

#[test]
fn select_on_a_scalar() {
    insta::assert_snapshot!(err("1 | select(. >= 2)"));
}
