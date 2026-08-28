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

/// The hint follows the payload's spelling: parens for a scalar, where `circle{...}` would
/// point at syntax the variant does not have.
#[test]
fn a_scalar_payload_variant_used_bare() {
    insta::assert_snapshot!(err("enum Temp { unknown, celsius(Int) }\n\ncelsius"));
}

#[test]
fn a_scalar_payload_of_the_wrong_type() {
    insta::assert_snapshot!(err("enum Temp { unknown, celsius(Int) }\n\ncelsius(\"hot\")"));
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

#[track_caller]
fn run_err(src: &str, stdin: &str) -> String {
    toylang::run_with_input(src, Some(stdin)).map(|_| ()).unwrap_err().to_string()
}

const FLIP: &str =
    "enum Status { active, inactive }\n\nfn f(s: Status) -> Status = s\n\nf(input)";

/// A wire mismatch names the enum, since "found a string" alone would not say which closed set
/// the string missed.
#[test]
fn input_that_is_no_variant_names_the_enum() {
    insta::assert_snapshot!(run_err(FLIP, "\"frozen\""));
}

/// The two wire shapes are not interchangeable: a payload variant's bare name is not a value
/// of it, and a unit variant wrapped in an object is not one either.
#[test]
fn input_using_the_wrong_shape_for_a_variant() {
    let src = "enum Shape { point, circle{r: Int} }\n\nfn f(s: Shape) -> Shape = s\n\nf(input)";
    insta::assert_snapshot!(format!(
        "{}\n{}",
        run_err(src, "\"circle\""),
        run_err(src, "{\"point\": 1}")
    ));
}

/// The proof has to say what is missing, not only that something is.
#[test]
fn a_match_that_misses_variants_names_them() {
    insta::assert_snapshot!(err(
        "enum Shape { point, circle{r: Int}, square{s: Int} }\n\nShape.point | point -> 0"
    ));
}

#[test]
fn an_arm_for_a_variant_the_enum_does_not_have() {
    insta::assert_snapshot!(err(
        "enum Shape { point }\n\nShape.point | square -> 1 // any() -> 0"
    ));
}

#[test]
fn an_arm_after_the_default_can_never_match() {
    insta::assert_snapshot!(err(
        "enum Shape { point, circle{r: Int} }\n\nShape.point | any() -> 0 // point -> 1"
    ));
}

/// Leaving payload fields out of a pattern is a forgotten field until `..` says it was meant --
/// the closed-type half of the sketch's subset rule.
#[test]
fn a_pattern_naming_only_some_payload_fields() {
    insta::assert_snapshot!(err(
        "enum P { xy{x: Int, y: Int} }\n\nxy{x: 1, y: 2} | xy{x} -> x"
    ));
}

#[test]
fn a_unit_variant_has_nothing_to_destructure() {
    insta::assert_snapshot!(err("enum S { a, b }\n\nS.a | a{q} -> 1 // b -> 2"));
}

/// A scalar payload has no fields for a `{...}` pattern to name; the arm's `.` is already the
/// payload, so the error points there.
#[test]
fn a_fields_pattern_on_a_scalar_payload() {
    insta::assert_snapshot!(err(
        "enum Temp { unknown, celsius(Int) }\n\ncelsius(21) | celsius{deg} -> deg // unknown -> 0"
    ));
}

#[test]
fn a_match_needs_an_enum_subject() {
    insta::assert_snapshot!(err("enum S { a }\n\n1 | a -> 2 // any() -> 3"));
}

#[test]
fn a_match_needs_a_subject_at_all() {
    insta::assert_snapshot!(err("enum S { a }\n\na -> 2"));
}

/// A unit arm rebinds `.` to nothing: there is no payload to reach, and the wider subject is
/// deliberately not reachable past the match.
#[test]
fn the_subject_is_not_reachable_inside_a_unit_arm() {
    insta::assert_snapshot!(err("enum S { a }\n\nS.a | a -> ."));
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
