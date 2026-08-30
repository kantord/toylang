fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn filter_a_vec() {
    insta::assert_snapshot!(toylang::run("[1, 2, 3] | select(. >= 2)").unwrap());
}

/// `v[]` with nothing after it is the identity: "keep every entry" is what a Vec already is.
#[test]
fn a_spec_with_nothing_to_spec_is_the_identity() {
    insta::assert_snapshot!(toylang::run("[1, 2, 3][] | select(. >= 2)").unwrap());
}

/// Every dimension needs a spec, so reaching a field through one without saying so fails.
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

/// `+` on a Vec now means concatenation (kantord/toylang#97), but only against another Vec of
/// the same element type -- against a Str this is an ordinary type mismatch, the same as any
/// other operand pair that disagrees.
#[test]
fn an_operator_does_not_apply_to_a_vec() {
    insta::assert_snapshot!(err(r#"[1, 2] + "a""#));
}

/// `+`'s carve-out does not extend to the rest of Q2: every other operator over two Vecs is
/// still refused.
#[test]
fn a_non_add_operator_still_does_not_apply_to_a_vec() {
    insta::assert_snapshot!(err("[1, 2] - [3, 4]"));
}

/// And a Vec one level down is the same open question: structural equality would otherwise
/// have to say whether the field compares as a whole value, which is what Q2 asks.
#[test]
fn equality_does_not_reach_past_a_vec() {
    insta::assert_snapshot!(err("{a: [1, 2]} == {a: [1, 2]}"));
}

/// The refusal walks enum payloads too, not only record fields.
#[test]
fn equality_does_not_reach_past_a_vec_in_a_payload() {
    insta::assert_snapshot!(err(r#"
enum Holder { empty, full{items: Vec<Int>} }

full{items: [1]} == full{items: [1]}
"#));
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
