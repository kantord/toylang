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
    let src = "fn nothing(x: Int) -> Vec<Int> = x | []\n\nnothing(1)";
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
    let src = "enum Status { Active, Inactive }\n\nfn initial(x: Int) -> Status = x | \"active\"\n\ninitial(1)";
    assert!(matches!(body_ty(src), Type::Enum { name, .. } if name == "Status"));
}

/// A payload variant cannot be built from its bare name; the hint spells the payload form.
#[test]
fn a_string_naming_a_payload_variant_is_refused() {
    insta::assert_snapshot!(err(
        "enum Shape { Point, Circle{r: Int} }\n\nfn f(x: Int) -> Shape = \"circle\"\n\nf(1)"
    ));
}

#[test]
fn a_string_naming_no_variant_is_refused() {
    insta::assert_snapshot!(err(
        "enum Status { Active, Inactive }\n\nfn f(x: Int) -> Status = \"gone\"\n\nf(1)"
    ));
}

/// Expectation resolves what synthesis refused; it never papers over a real mismatch, and the
/// mismatch error still blames the function by name.
#[test]
fn a_synthesised_mismatch_still_names_the_function() {
    insta::assert_snapshot!(err("fn f(x: Int) -> Vec<Int> = x | \"a\"\n\nf(1)"));
}

// Step 2: a record literal checked against a record type pushes each field's expected type
// into its value, and `input`'s first checked use fixes its type for every later one.

#[test]
fn record_fields_receive_the_declared_types() {
    let src = "enum Status { Active, Inactive }\n\n\
               fn f(x: Int) -> {v: Vec<Int>, s: Status} = x | {v: [], s: \"active\"}\n\nf(1)";
    assert!(matches!(body_ty(src), Type::Record(_)));
}

#[test]
fn input_in_a_field_checked_against_a_declared_record() {
    let src = "fn f(x: Int) -> {n: Int} = x | {n: input}\n\nf(1)";
    let program = toylang::compile(src).unwrap();
    assert_eq!(program.input, Some(Type::Int));
}

#[test]
fn a_later_input_borrows_the_type_the_first_use_fixed() {
    let src = "fn f(v: Vec<Int>) -> Int = length(v)\n\n{a: f(input), b: input}";
    assert_eq!(
        body_ty(src),
        Type::Record(vec![
            ("a".to_string(), Type::Int),
            ("b".to_string(), Type::Vec(Box::new(Type::Int))),
        ])
    );
}

/// First use wins is an order, not a unification: an `input` ahead of every typed use has
/// nothing to borrow.
#[test]
fn an_input_ahead_of_every_typed_use_is_still_refused() {
    insta::assert_snapshot!(err(
        "fn f(v: Vec<Int>) -> Int = length(v)\n\n{a: input, b: f(input)}"
    ));
}

/// A shuffled literal is one type with the fields spelled in order (kantord/toylang#60), so it
/// checks against a differently-ordered return type -- and comes out re-fielded to the
/// signature's declared order, which is what a caller reading the value relies on.
#[test]
fn reordered_fields_check_against_the_return_type() {
    let program =
        toylang::compile("fn f(x: Int) -> {a: Int, b: Str} = x | {b: \"x\", a: 1}\n\nf(1)")
            .unwrap();
    let Type::Record(fields) = &program.funcs[0].body.ty else {
        panic!("expected a record type");
    };
    assert_eq!(
        fields,
        &vec![("a".to_string(), Type::Int), ("b".to_string(), Type::Str),]
    );
}

#[test]
fn a_missing_field_still_mismatches() {
    insta::assert_snapshot!(err("fn f(x: Int) -> {a: Int, b: Str} = x | {a: 1}\n\nf(1)"));
}

// Step 3: a call against a known signature pushes the parameter type into the argument. The
// position pushed before this rework; these pin that the new checked forms resolve there.

#[test]
fn empty_vec_as_an_argument() {
    let src = "fn f(v: Vec<Int>) -> Int = length(v)\n\nf([])";
    assert_eq!(body_ty(src), Type::Int);
}

#[test]
fn a_string_names_a_variant_in_argument_position() {
    let src = "enum Status { Active, Inactive }\n\n\
               fn flip(s: Status) -> Status =\n    \
               s | Active -> Status.inactive or Inactive -> Status.active\n\n\
               flip(\"active\")";
    assert!(matches!(body_ty(src), Type::Enum { name, .. } if name == "Status"));
}

#[test]
fn input_in_a_record_argument_field() {
    let src = "fn g(r: {n: Int, tag: Str}) -> Int = r.n\n\ng({n: input, tag: \"x\"})";
    let program = toylang::compile(src).unwrap();
    assert_eq!(program.input, Some(Type::Int));
}

/// A constructor payload is checked against the declared payload type, so the checked forms
/// resolve inside one too.
#[test]
fn empty_vec_in_a_constructor_payload() {
    let src = "enum Box { Of{items: Vec<Int>} }\n\nof{items: []}";
    assert!(matches!(body_ty(src), Type::Enum { name, .. } if name == "Box"));
}

/// `length` is polymorphic over its element type, so its argument is synthesised: there is no
/// declared parameter type to flow in, and `[]` stays unknowable there.
#[test]
fn a_polymorphic_builtin_still_synthesises_its_argument() {
    insta::assert_snapshot!(err("length([])"));
}

// Step 4: the expectation flows through `|` into the right side, and a `map` whose position
// expects a matching cardinality pushes the expected element type into its body.

#[test]
fn the_expectation_flows_through_a_pipe() {
    let src = "fn f(x: Int) -> Vec<Int> = x | [.]\n\nf(1)";
    assert_eq!(body_ty(src), Type::Vec(Box::new(Type::Int)));
}

#[test]
fn empty_vec_in_a_map_body() {
    let src = "fn pad(v: Vec<Int>) -> Vec<Vec<Int>> = v | map([])\n\npad([1, 2])";
    assert_eq!(
        body_ty(src),
        Type::Vec(Box::new(Type::Vec(Box::new(Type::Int))))
    );
}

/// The parse-shaped body the rejected `parse` design needed: the element the map must produce
/// is declared, and each body form checks against it.
#[test]
fn a_record_map_body_takes_the_declared_element() {
    let src = "enum Status { Active, Inactive }\n\n\
               fn tag(v: Vec<Int>) -> Vec<{n: Int, s: Status}> = v | map({n: ., s: \"active\"})\n\n\
               tag([1])";
    assert!(matches!(body_ty(src), Type::Vec(_)));
}

#[test]
fn a_stream_map_body_takes_the_declared_element() {
    let src = "fn pad(s: Stream<Int>) -> Stream<Vec<Int>> = s | map([])\n\n\
               collect(pad(inputs))";
    assert!(matches!(body_ty(src), Type::Vec(_)));
}

/// The pushed element reaches the body, so the mismatch is the body's, not the whole map's.
#[test]
fn a_map_body_that_misses_the_element_type() {
    insta::assert_snapshot!(err(
        "fn f(v: Vec<Int>) -> Vec<Str> = v | map(. + 1)\n\nf([1])"
    ));
}

// Step 5: both branches of a conditional and every arm of a total match chain receive the
// expectation. A partial guard chain receives it too, peeled: a declared Opt<T> return pushes
// T into the arms, since the chain's own partiality is what supplies the Opt (kantord/toylang#48).

#[test]
fn both_conditional_branches_receive_the_expectation() {
    let src = "fn f(x: Int) -> Vec<Int> = [] if x > 0 else [1]\n\nf(1)";
    assert_eq!(body_ty(src), Type::Vec(Box::new(Type::Int)));
}

#[test]
fn conditional_branches_can_name_variants() {
    let src = "enum Status { Active, Inactive }\n\n\
               fn status(n: Int) -> Status = \"active\" if n > 0 else \"inactive\"\n\n\
               status(0)";
    assert!(matches!(body_ty(src), Type::Enum { name, .. } if name == "Status"));
}

/// The annotation decides which branch is wrong, so the error lands on the branch that
/// misses it rather than on whichever branch came second.
#[test]
fn the_branch_that_misses_the_annotation_is_blamed() {
    insta::assert_snapshot!(err("fn f(x: Int) -> Str = 1 if x > 0 else \"a\"\n\nf(1)"));
}

#[test]
fn match_arms_receive_the_expectation() {
    let src = "enum Status { Active, Inactive }\n\n\
               fn work(s: Status) -> Vec<Int> = s | Active -> [1, 2] or Inactive -> []\n\n\
               work(\"inactive\")";
    assert_eq!(body_ty(src), Type::Vec(Box::new(Type::Int)));
}

#[test]
fn a_default_arm_receives_the_expectation_too() {
    let src = "fn f(x: Int) -> Vec<Int> = x | . > 0 -> [x] or any() -> []\n\nf(1)";
    assert_eq!(body_ty(src), Type::Vec(Box::new(Type::Int)));
}

/// A pure guard chain may decline every arm, so its own result is `Opt` of whatever the arms
/// yield (kantord/toylang#62). A declared `Opt<T>` return therefore peels to `T` before it
/// reaches the arms (kantord/toylang#48, closing the conservative cut #62 left open), which is
/// what lets `[]` resolve inside a partial arm the same way it does anywhere else `T` is wanted.
#[test]
fn a_partial_chain_peels_the_declared_opt_into_its_arms() {
    let src = "fn f(x: Int) -> Opt<Vec<Int>> = x | . > 0 -> []\n\nf(1)";
    assert_eq!(body_ty(src).to_string(), "Opt<Vec<Int>>");
}

/// The peeled expectation still blames the arm that misses it, by name, the same as any other
/// checked position.
#[test]
fn a_partial_chain_arm_that_misses_the_peeled_element() {
    insta::assert_snapshot!(err(
        "fn f(x: Int) -> Opt<Vec<Int>> = x | . > 0 -> [\"a\"]\n\nf(1)"
    ));
}

/// An arm whose body is naturally `Opt`-typed (an index into a `Vec`) is not auto-unwrapped by
/// the peel: it is checked against the peeled `T`, so it still needs the doubled annotation to
/// carry its own `Opt` on top of the chain's.
#[test]
fn a_partial_chain_over_opt_arms_still_wants_the_doubled_annotation() {
    let doubled = "fn f(v: Vec<Int>) -> Opt<Opt<Int>> = v | length(v) > 0 -> v[9]\n\nf([1])";
    assert!(toylang::compile(doubled).is_ok());
    insta::assert_snapshot!(err(
        "fn f(v: Vec<Int>) -> Opt<Int> = v | length(v) > 0 -> v[9]\n\nf([1])"
    ));
}

/// Exhaustiveness is unchanged by the expectation: covering every variant is still the arms'
/// own duty.
#[test]
fn an_uncovered_variant_is_still_refused_under_an_expectation() {
    insta::assert_snapshot!(err("enum Status { Active, Inactive }\n\n\
         fn f(s: Status) -> Vec<Int> = s | Active -> []\n\nf(\"active\")"));
}

// The stream-containment lesson survives the checked fast paths (found landing this branch:
// the pushed-expectation arms demoted it to a bare mismatch).

#[test]
fn a_stream_in_a_checked_record_field_keeps_its_refusal() {
    insta::assert_snapshot!(err("fn f(v: Stream<Int>) -> {n: Int} = {n: v}\n\n1"));
}

#[test]
fn a_stream_in_a_checked_vec_keeps_its_refusal() {
    insta::assert_snapshot!(err("fn f(v: Stream<Int>) -> Vec<Int> = [v]\n\n1"));
}
