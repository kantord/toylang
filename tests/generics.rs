//! Generic enums: type parameters on declarations, `Name<...>` instantiation, and the
//! constructor inference that binds parameters from a payload (plans/opt-as-enum.md step 1).
//!
//! The corpus carries the positive behaviour on every backend (the generic_enum_* cases).
//! These pin the checker's refusals, which no backend ever sees, and the module form.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn a_generic_enum_needs_its_argument() {
    insta::assert_snapshot!(err(
        "enum Pair<T> { two{a: T, b: T} }\n\nfn f(p: Pair) -> Int = 1\n\nf(two{a: 1, b: 2})"
    ));
}

#[test]
fn a_generic_enum_refuses_extra_arguments() {
    insta::assert_snapshot!(err(
        "enum Box<T> { wrap(T), empty }\n\nfn f(b: Box<Int, Str>) -> Int = 1\n\nf(wrap(1))"
    ));
}

#[test]
fn a_plain_enum_takes_no_argument() {
    insta::assert_snapshot!(err(
        "enum Shape { point }\n\nfn f(s: Shape<Int>) -> Int = 1\n\nf(point)"
    ));
}

#[test]
fn a_builtin_scalar_takes_no_argument() {
    insta::assert_snapshot!(err("fn f(s: Str<Int>) -> Int = 1\n\nf(\"x\")"));
}

#[test]
fn an_alias_takes_no_argument() {
    insta::assert_snapshot!(err(
        "type Db = {n: Int}\n\nfn f(d: Db<Int>) -> Int = 1\n\nf({n: 1})"
    ));
}

#[test]
fn a_type_parameter_is_capitalized() {
    insta::assert_snapshot!(err("enum Box<t> { wrap(t) }\n\nstr(1)"));
}

#[test]
fn a_type_parameter_declared_twice() {
    insta::assert_snapshot!(err("enum Pair<T, T> { two{a: T, b: T} }\n\nstr(1)"));
}

#[test]
fn a_type_parameter_cannot_take_a_builtin_name() {
    insta::assert_snapshot!(err("enum Box<Int> { wrap(Int) }\n\nstr(1)"));
}

/// A parameter shadows a declared name inside its own declaration -- resolve_named consults
/// the bindings first, so `Shape` in the payload means the parameter (kantord/toylang#85:
/// the old refusal broke every `enum E` program when the prelude gained Result<T, E>).
#[test]
fn a_type_parameter_shadows_a_declared_name() {
    let src = "enum Shape { point }\nenum Box<Shape> { wrap(Shape) }\n\nstr(1)";
    assert!(toylang::compile(src).is_ok());
}

#[test]
fn a_type_parameter_takes_no_argument() {
    insta::assert_snapshot!(err("enum Box<T> { wrap(T<Int>) }\n\nstr(1)"));
}

#[test]
fn a_stream_cannot_be_a_type_argument() {
    insta::assert_snapshot!(err(
        "enum Box<T> { wrap(T), empty }\n\nfn f(b: Box<Stream<Str>>) -> Int = 1\n\nf(empty)"
    ));
}

/// The `[]` problem again: nothing about a bare unit variant says what the arguments are, so
/// only a position that expects a known instantiation can build one. The corpus case
/// generic_enum_unit_expectation is the spelling that works.
#[test]
fn a_bare_unit_variant_of_a_generic_enum_cannot_be_synthesised() {
    insta::assert_snapshot!(err("enum Box<T> { wrap(T), empty }\n\nempty"));
}

/// A payload that does not mention every parameter leaves the instantiation open the same
/// way, even though a payload was written.
#[test]
fn a_payload_that_leaves_a_parameter_open() {
    insta::assert_snapshot!(err("enum Weird<T> { w(Int), v(T) }\n\nw(1)"));
}

/// One parameter bound two ways is a mismatch inside the payload, reported against the
/// declared payload type with the parameter still visible in it.
#[test]
fn a_parameter_bound_two_ways() {
    insta::assert_snapshot!(err(
        "enum Pair<T> { two{a: T, b: T} }\n\ntwo{a: 1, b: \"x\"}"
    ));
}

#[test]
fn a_recursive_generic_payload_is_still_a_cycle() {
    insta::assert_snapshot!(err(
        "enum List<T> { nil, cons{head: T, tail: List<T>} }\n\nstr(1)"
    ));
}

/// A boxed self-reference that re-parameterizes rather than repeating its own arguments names
/// an infinite family of instantiations -- `Nest<T>`, `Nest<Vec<T>>`, `Nest<Vec<Vec<T>>>`, ...
/// none of which ever recur -- so every walk that dedupes by full type equality (variant
/// listing, recursion detection, codegen) diverges the moment something forces one open
/// (kantord/toylang#117). Refused here, before any of those walks run.
#[test]
fn a_reparameterized_self_reference_is_refused() {
    insta::assert_snapshot!(err(
        "enum Nest<T> { one, wrap(Vec<Nest<Vec<T>>>) }\n\nstr(1)"
    ));
}

/// The declaration parses in a module with its parameters, `pub` and all -- the form the
/// prelude's `Opt<T>` will use (plans/opt-as-enum.md step 2).
#[test]
fn a_generic_enum_parses_in_a_module() {
    let module = toylang::parse::parse_module("pub enum Opt2<T> { some(T), none }\n").unwrap();
    assert_eq!(module.enums.len(), 1);
    assert_eq!(module.enums[0].params.len(), 1);
    assert_eq!(module.enums[0].params[0].0, "T");
    assert!(module.enums[0].is_pub);
}
