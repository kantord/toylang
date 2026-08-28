//! The jq backend against the things it alone can say.
//!
//! Behaviour lives in the corpus like every other backend. What is here is the two rules jq
//! forced that the others did not.

const FORWARD: &str = r#"
fn outer(x: Str) -> Str = inner(x) + "!"
fn inner(x: Str) -> Str = "[" + x + "]"

outer("hi")
"#;

/// jq resolves a `def` only against what is already defined and has no forward declaration, so
/// definitions come out callee-first. The checker accepts the other order, which is a rule this
/// target does not share.
#[test]
fn definitions_come_out_callee_first() {
    let p = toylang::compile(FORWARD).unwrap();
    let jq = toylang::emit_jq::emit(&p);
    let inner = jq.find("def v_inner").expect("inner is defined");
    let outer = jq.find("def v_outer").expect("outer is defined");
    assert!(inner < outer, "callee must be defined first:\n{jq}");
    insta::assert_snapshot!(jq);
}

/// jq's -r decides from the runtime value, so it would print a present Opt<Str> raw and an
/// absent one as the word null. The rule here is the type's, as on every other backend.
#[test]
fn an_optional_string_prints_as_json() {
    insta::assert_snapshot!(
        toylang::run_on(r#"["ada", "bo"][0]"#, None, toylang::Backend::Jq).unwrap()
    );
}
