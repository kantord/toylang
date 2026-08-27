//! Enums: declarations, constructors, printing (plans/enums.md step 1).
//!
//! The corpus carries the positive behaviour -- construction and printing run on every backend
//! there. These pin what the corpus cannot see: the checker's refusals, and the module form,
//! which has no program around it to run.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn a_variant_declared_twice_in_one_enum() {
    insta::assert_snapshot!(err("enum Shape { point, point }\n\nstr(1)"));
}

/// The error has to name both candidates: knowing the name is ambiguous is useless without
/// knowing what to qualify it with.
#[test]
fn a_bare_variant_two_enums_declare() {
    insta::assert_snapshot!(err(
        "enum Status { active, inactive }\nenum Toggle { active, off }\n\nactive"
    ));
}

#[test]
fn a_qualified_variant_the_enum_does_not_have() {
    insta::assert_snapshot!(err("enum Shape { point, circle{r: Int} }\n\nShape.square"));
}

#[test]
fn a_payload_handed_to_a_unit_variant() {
    insta::assert_snapshot!(err("enum Shape { point, circle{r: Int} }\n\npoint{r: 1}"));
}

#[test]
fn a_payload_variant_used_bare() {
    insta::assert_snapshot!(err("enum Shape { point, circle{r: Int} }\n\ncircle"));
}

/// Expanding this would not terminate: there is no indirection for a recursive payload to hide
/// behind, so it is refused the way a recursive alias is.
#[test]
fn an_enum_whose_payload_mentions_itself() {
    insta::assert_snapshot!(err("enum E { leaf, node{next: E} }\n\nleaf"));
}

/// Aliases and enums share the type namespace, so the same name twice is the same error either
/// way around.
#[test]
fn an_enum_cannot_reuse_an_alias_name() {
    insta::assert_snapshot!(err("type A = Int\nenum A { x }\n\nstr(1)"));
}

/// Validating a wire value against an enum is step 5 of plans/enums.md; until every backend can
/// do it, the checker refuses rather than letting seven readers disagree.
#[test]
fn enum_typed_input_is_refused_for_now() {
    insta::assert_snapshot!(err(
        "enum Status { active, inactive }\n\nfn f(s: Status) -> Status = s\n\nf(input)"
    ));
}

/// The declaration parses in a module the same as in a program, `pub` and all; nothing in the
/// prelude declares one yet, so the module form is witnessed here rather than by a corpus case.
#[test]
fn an_enum_parses_in_a_module() {
    let module = toylang::parse::parse_module(
        "pub enum Status { active, inactive }\n\npub fn same(s: Status) -> Status = s\n",
    )
    .unwrap();
    assert_eq!(module.enums.len(), 1);
    assert_eq!(module.enums[0].name, "Status");
    assert!(module.enums[0].is_pub);
}
