fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn filter_a_vec() {
    insta::assert_snapshot!(toylang::run("[1, 2, 3][] | select(. >= 2)").unwrap());
}

/// Under C1 a projection by every index keeps the same extent, so `[]` is the identity on a Vec
/// and these two programs are the same program. Asserting it here so that if `[]` ever stops
/// being a no-op, something goes red.
#[test]
fn projection_is_the_identity() {
    let with = toylang::compile("[1, 2, 3][] | select(. >= 2)").unwrap().0;
    let without = toylang::compile("[1, 2, 3] | select(. >= 2)").unwrap().0;
    assert_eq!(with, without);
    insta::assert_snapshot!(with);
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
    let (lua, ty) = toylang::compile("[1, 2, 3][] | select(. >= 2)").unwrap();
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

#[test]
fn project_a_non_vec() {
    insta::assert_snapshot!(err("1[]"));
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
