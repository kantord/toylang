//! The top-down type flow rework (plans/type-flow.md): declared types resolve `[]` and
//! literals in bodies. Each step of the rework pins its behavior here -- what newly compiles,
//! what stays refused, and that expectation never overrides a successful synthesis.

use toylang::ty::Type;

fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

fn body_ty(src: &str) -> Type {
    toylang::compile(src).unwrap().body.ty
}

// Step 1: the declared return type flows into the function body.

#[test]
fn empty_vec_resolves_against_the_return_type() {
    let src = "fn nothing(x: Int) -> Vec<Int> = []\n\nnothing(1)";
    assert_eq!(body_ty(src), Type::Vec(Box::new(Type::Int)));
}

#[test]
fn empty_vec_nested_in_a_literal_resolves_too() {
    let src = "fn f(x: Int) -> Vec<Vec<Int>> = [[], [x]]\n\nf(1)";
    assert_eq!(
        body_ty(src),
        Type::Vec(Box::new(Type::Vec(Box::new(Type::Int))))
    );
}

#[test]
fn a_bare_empty_vec_is_still_refused() {
    insta::assert_snapshot!(err("[]"));
}

/// The expectation reaches each element, so the error names the element type and points at
/// the entry, not at the whole literal.
#[test]
fn wrong_element_under_annotation() {
    insta::assert_snapshot!(err("fn f(x: Int) -> Vec<Int> = [\"a\"]\n\nf(1)"));
}

#[test]
fn a_string_names_a_unit_variant_in_return_position() {
    let src = "enum Status { active, inactive }\n\nfn initial(x: Int) -> Status = \"active\"\n\ninitial(1)";
    assert!(matches!(body_ty(src), Type::Enum { name, .. } if name == "Status"));
}

/// A payload variant cannot be built from its bare name; the hint spells the payload form.
#[test]
fn a_string_naming_a_payload_variant_is_refused() {
    insta::assert_snapshot!(err(
        "enum Shape { point, circle{r: Int} }\n\nfn f(x: Int) -> Shape = \"circle\"\n\nf(1)"
    ));
}

#[test]
fn a_string_naming_no_variant_is_refused() {
    insta::assert_snapshot!(err(
        "enum Status { active, inactive }\n\nfn f(x: Int) -> Status = \"gone\"\n\nf(1)"
    ));
}

/// Expectation resolves what synthesis refused; it never papers over a real mismatch, and the
/// mismatch error still blames the function by name.
#[test]
fn a_synthesised_mismatch_still_names_the_function() {
    insta::assert_snapshot!(err("fn f(x: Int) -> Vec<Int> = \"a\"\n\nf(1)"));
}
