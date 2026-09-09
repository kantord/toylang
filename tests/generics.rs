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
        "enum Pair<T> { Two{a: T, b: T} }\n\nfn f(p: Pair) -> Int = 1\n\nf(two{a: 1, b: 2})"
    ));
}

#[test]
fn a_generic_enum_refuses_extra_arguments() {
    insta::assert_snapshot!(err(
        "enum Box<T> { Wrap(T), Empty }\n\nfn f(b: Box<Int, Str>) -> Int = 1\n\nf(wrap(1))"
    ));
}

#[test]
fn a_plain_enum_takes_no_argument() {
    insta::assert_snapshot!(err(
        "enum Shape { Point }\n\nfn f(s: Shape<Int>) -> Int = 1\n\nf(point)"
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
    insta::assert_snapshot!(err("enum Box<t> { Wrap(t) }\n\nstr(1)"));
}

#[test]
fn a_type_parameter_declared_twice() {
    insta::assert_snapshot!(err("enum Pair<T, T> { Two{a: T, b: T} }\n\nstr(1)"));
}

#[test]
fn a_type_parameter_cannot_take_a_builtin_name() {
    insta::assert_snapshot!(err("enum Box<Int> { Wrap(Int) }\n\nstr(1)"));
}

/// A parameter shadows a declared name inside its own declaration -- resolve_named consults
/// the bindings first, so `Shape` in the payload means the parameter (kantord/toylang#85:
/// the old refusal broke every `enum E` program when the prelude gained Result<T, E>).
#[test]
fn a_type_parameter_shadows_a_declared_name() {
    let src = "enum Shape { Point }\nenum Box<Shape> { Wrap(Shape) }\n\nstr(1)";
    assert!(toylang::compile(src).is_ok());
}

#[test]
fn a_type_parameter_takes_no_argument() {
    insta::assert_snapshot!(err("enum Box<T> { Wrap(T<Int>) }\n\nstr(1)"));
}

#[test]
fn a_stream_cannot_be_a_type_argument() {
    insta::assert_snapshot!(err(
        "enum Box<T> { Wrap(T), Empty }\n\nfn f(b: Box<Stream<Str>>) -> Int = 1\n\nf(empty)"
    ));
}

/// The `[]` problem again: nothing about a bare unit variant says what the arguments are, so
/// only a position that expects a known instantiation can build one. The corpus case
/// generic_enum_unit_expectation is the spelling that works.
#[test]
fn a_bare_unit_variant_of_a_generic_enum_cannot_be_synthesised() {
    insta::assert_snapshot!(err("enum Box<T> { Wrap(T), Empty }\n\nempty"));
}

/// A payload that does not mention every parameter leaves the instantiation open the same
/// way, even though a payload was written.
#[test]
fn a_payload_that_leaves_a_parameter_open() {
    insta::assert_snapshot!(err("enum Weird<T> { W(Int), V(T) }\n\nw(1)"));
}

/// One parameter bound two ways is a mismatch inside the payload, reported against the
/// declared payload type with the parameter still visible in it.
#[test]
fn a_parameter_bound_two_ways() {
    insta::assert_snapshot!(err(
        "enum Pair<T> { Two{a: T, b: T} }\n\ntwo{a: 1, b: \"x\"}"
    ));
}

#[test]
fn a_recursive_generic_payload_is_still_a_cycle() {
    insta::assert_snapshot!(err(
        "enum List<T> { Nil, Cons{head: T, tail: List<T>} }\n\nstr(1)"
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
        "enum Nest<T> { One, Wrap(Vec<Nest<Vec<T>>>) }\n\nstr(1)"
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

/// A module can carry a trait declaration, an impl block, and a type alias the same way a program
/// file can -- the prelude will need to ship all three once the trait scaffold lands.
#[test]
fn trait_impl_and_alias_parse_in_a_module() {
    let module = toylang::parse::parse_module("trait T {}\nimpl T for S {}\ntype A = B\n").unwrap();
    assert_eq!(module.traits.len(), 1);
    assert_eq!(module.impls.len(), 1);
    assert_eq!(module.aliases.len(), 1);
}

/// The trait scaffold:an impl block's methods are checked against the trait's signatures (with
/// `Self` substituted by the impl's target type) and synthesized into ordinary functions, the
/// same path a hand-written prelude `fn` takes -- named `"{method}::{TypeName}"`
/// (`check::collect_impls`) rather than by the bare method name, so two impls of different
/// types can share a method name without colliding.
#[test]
fn an_impl_block_synthesizes_its_methods_as_functions() {
    let module = toylang::parse::parse_module(
        "trait Fold {\n    fn identity() -> Self\n    fn step(p: {acc: Self, x: Int}) -> Self\n}\n\nimpl Fold for Vec<Int> {\n    fn identity() -> Self = []\n    fn step(p: {acc: Self, x: Int}) -> Self = p.acc + [p.x]\n}\n",
    ).unwrap();
    let (funcs, _) = toylang::check::check_module(module).unwrap();
    assert_eq!(
        funcs.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        vec!["identity::Vec_Int", "step::Vec_Int"]
    );
}

/// An impl method must repeat its trait's signature exactly,with `Self` meaning the impl's
/// target type;a method that re-types one is refused. The trait is what the impl is checked
/// against, not just a name the impl borrows.

#[test]
fn an_impl_method_must_match_its_trait_signature() {
    let module = toylang::parse::parse_module(
        "trait Fold {\n    fn identity() -> Self\n}\n\nimpl Fold for Vec<Int> {\n    fn identity() -> Int = 0\n}\n",
    ).unwrap();
    insta::assert_snapshot!(
        toylang::check::check_module(module)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

/// A colon call dispatches by the receiver's concrete type; a type with no impl of the named
/// trait is refused rather than silently reaching some other impl.
#[test]
fn a_colon_call_with_no_matching_impl_is_refused() {
    insta::assert_snapshot!(err(
        "trait Area {\n    fn area(s: Self) -> Int\n}\n\ntype Circle = {r: Int}\n\nimpl Area for Circle {\n    fn area(s: Self) -> Int = s.r * s.r\n}\n\n3:area()"
    ));
}

/// An impl's methods are exactly its trait's; one that is not among them is refused before the
/// signature-match check ever runs, the same as calling a method a trait never declared.
#[test]
fn an_impl_method_not_named_by_its_trait_is_refused() {
    insta::assert_snapshot!(err(
        "trait Area {\n    fn area(s: Self) -> Int\n}\n\ntype Circle = {r: Int}\n\nimpl Area for Circle {\n    fn area(s: Self) -> Int = s.r * s.r\n    fn perimeter(s: Self) -> Int = s.r * 4\n}\n\n1"
    ));
}

/// `x:foo(y)` on a plain function is UFCS sugar for `foo(x)`, which has no room left for a
/// separate `y`: no unary function can take a receiver and an argument at once (gh:174).
#[test]
fn a_plain_function_colon_called_with_an_argument_is_refused() {
    insta::assert_snapshot!(err("fn double(x: Int) -> Int = x * 2\n\n3:double(4)"));
}

/// The plain-function namespace and the trait-method namespace cannot share a name: the old
/// `collect_impls` got this "for free" as an accidental collision when it wrote impl methods
/// into the same flat map plain functions used; the rewrite states it as an explicit check.
#[test]
fn an_impl_method_colliding_with_a_plain_function_is_refused() {
    insta::assert_snapshot!(err(
        "fn area(x: Int) -> Int = x\n\ntrait Area {\n    fn area(s: Self) -> Int\n}\n\ntype Circle = {r: Int}\n\nimpl Area for Circle {\n    fn area(s: Self) -> Int = s.r * s.r\n}\n\n1"
    ));
}

/// Two different traits' impls for the *same* concrete type still collide on a shared method
/// name -- this is the actual bug the rewrite fixes: different *types* sharing a method name no
/// longer collides (see `an_impl_block_synthesizes_its_methods_as_functions`'s two mangled
/// names), but the same type genuinely cannot have two methods named the same regardless of
/// which trait either comes from.
#[test]
fn two_impls_of_different_traits_for_the_same_type_and_method_collide() {
    insta::assert_snapshot!(err(
        "trait Area {\n    fn area(s: Self) -> Int\n}\n\ntrait Size {\n    fn area(s: Self) -> Int\n}\n\ntype Circle = {r: Int}\n\nimpl Area for Circle {\n    fn area(s: Self) -> Int = s.r * s.r\n}\n\nimpl Size for Circle {\n    fn area(s: Self) -> Int = s.r\n}\n\n1"
    ));
}

/// The `::` -> `__` backend escaping is not provably injective on its own: underscores already
/// inside a method or type name can make two distinct `(method, Type)` pairs collide once
/// escaped even though neither their method names nor their target types match. `foo__Ba` for
/// `R` and `foo` for `Ba__R` both escape to `foo__Ba__R`.
#[test]
fn two_impls_whose_escaped_names_collide_are_refused() {
    insta::assert_snapshot!(err(
        "enum R { RTag }\n\nenum Ba__R { BaRTag }\n\ntrait X {\n    fn foo__Ba(s: Self) -> Int\n}\n\ntrait Y {\n    fn foo(s: Self) -> Int\n}\n\nimpl X for R {\n    fn foo__Ba(s: Self) -> Int = 1\n}\n\nimpl Y for Ba__R {\n    fn foo(s: Self) -> Int = 2\n}\n\n1"
    ));
}
