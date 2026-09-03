//! What the corpus cannot express. Behaviour that must be identical across backends lives in
//! tests/corpus/ and is checked by tests/corpus.rs; this file holds the claims that need more
//! than one run of one program to state.

use toylang::Backend;

#[track_caller]
fn agree(src: &str, stdin: Option<&str>) -> String {
    let mut results = Backend::ALL
        .iter()
        .map(|b| (b.name(), toylang::run_on(src, stdin, *b).unwrap()));
    let (first_name, first) = results.next().expect("at least one backend");
    for (name, out) in results {
        assert_eq!(first, out, "{first_name} and {name} disagree on:\n{src}");
    }
    first
}

/// Record keys come out in the type's order on every backend, not in the order the input
/// happened to list them. Two inputs, one expected output, so a backend reverting to insertion
/// order fails here. A corpus entry has one input and cannot say this.
#[test]
fn record_key_order_follows_the_type() {
    let src = r#"
fn first(db: {u: {name: Str, age: Int}}) -> {name: Str, age: Int} = db.u

first(input)
"#;
    let declared_order = agree(src, Some(r#"{"u": {"name": "ada", "age": 36}}"#));
    let reversed = agree(src, Some(r#"{"u": {"age": 36, "name": "ada"}}"#));
    assert_eq!(declared_order, reversed);
    insta::assert_snapshot!(declared_order);
}

// Float is JS and Native only so far (kantord/toylang#149): jq, Go, Python, Rust, and Lua do
// not carry it yet, so none of these can be a corpus case, which would require all seven
// backends to agree. They live here, checked for JS/Native agreement, until the remaining
// follow-up rows bring Float to the rest of the backends -- at which point this block moves
// into the corpus.
#[track_caller]
fn agree_float(src: &str, stdin: Option<&str>) -> String {
    let js = toylang::run_on(src, stdin, Backend::Js).unwrap();
    let native = toylang::run_on(src, stdin, Backend::Native).unwrap();
    assert_eq!(js, native, "js and native disagree on:\n{src}");
    js
}

/// A decimal-point literal parses, type-checks, and prints through the printer.
#[test]
fn float_literals_parse_and_print() {
    assert_eq!(agree_float("1.5 + 0.25 * 2.0\n", None), "2\n");
}

/// Division on a Float is total (ADR 0007, the Q37 ruling): a zero divisor is the IEEE answer,
/// Infinity, not the stop an Int's `1 / 0` is.
#[test]
fn float_division_by_zero_is_infinity() {
    assert_eq!(agree_float("1.0 / 0.0\n", None), "Infinity\n");
}

/// NaN and Infinity are values a Float can hold, and both print as their names.
#[test]
fn float_nan_and_infinity_are_producible() {
    assert_eq!(agree_float("0.0 / 0.0\n", None), "NaN\n");
    assert_eq!(agree_float("-1.0 / 0.0\n", None), "-Infinity\n");
}

/// Float comparisons type-check and print a Bool, and NaN follows IEEE: not equal to itself,
/// and not less than, not greater than anything either.
#[test]
fn float_comparison() {
    assert_eq!(agree_float("1.5 < 2.5\n", None), "true\n");
    assert_eq!(agree_float("(0.0 / 0.0) == (0.0 / 0.0)\n", None), "false\n");
    assert_eq!(agree_float("(0.0 / 0.0) != (0.0 / 0.0)\n", None), "true\n");
}

/// A Float result keeps its value when read back off the wire: a JSON number already is the
/// double a Float names (ADR 0007), the case that made Int64 refuse `input`. Also covers an
/// integer-shaped JSON number arriving where Float was declared (`3`, not `3.0`), which is
/// still a legal Float per input.rs's own rule.
#[test]
fn float_input_reads_a_json_number() {
    let src = "fn twice(x: Float) -> Float = x * 2.0\n\ntwice(input)\n";
    assert_eq!(agree_float(src, Some("2.5")), "5\n");
    assert_eq!(agree_float(src, Some("3")), "6\n");
}

/// `-x` on a Float-typed variable, not a literal -- the checker folds `-3.5` straight into a
/// negative literal, so this is the only path that exercises the general `0 - x` desugaring at
/// Float width, which needed its own fix (`Kind::Float(0.0)`, not `Kind::Int(0)` typed as
/// Float) once a backend other than JS told the two Kinds apart.
#[test]
fn float_negation_of_a_variable() {
    let src = "fn neg(x: Float) -> Float = -x\n\nneg(3.5)\n";
    assert_eq!(agree_float(src, None), "-3.5\n");
}

/// A record and a Vec of Floats print and compare correctly, exercising the uniform 8-byte-slot
/// storage (a Float's bit pattern, not its value, is what a slot holds) and the composite
/// equality path, both separate from the scalar arithmetic/printing paths above.
#[test]
fn float_in_a_record_and_a_vec() {
    assert_eq!(
        agree_float("{a: 1.5, b: 2.5}\n", None),
        "{\"a\":1.5,\"b\":2.5}\n"
    );
    assert_eq!(agree_float("[1.5, 2.25, 3.0]\n", None), "[1.5,2.25,3]\n");
    assert_eq!(
        agree_float("{a: 1.5, b: 2.5} == {a: 1.5, b: 2.5}\n", None),
        "true\n"
    );
}

/// Printing format across the ECMA-262 Number::toString notation-switch boundaries: fixed vs
/// scientific at 1e21 and 1e-7/1e-6, and the trailing zeros an integer-valued Float still needs
/// suppressed. This is the one place Native's own formatter (runtime/toylang.c's
/// tl_float_to_str, verified in isolation against a ~5000-value fuzz run against Node before
/// landing) has real work to do that JS gets from its own runtime for free, so it is worth
/// covering past the arithmetic/comparison shapes above.
#[test]
fn float_printing_matches_at_notation_boundaries() {
    assert_eq!(agree_float("100.0\n", None), "100\n");
    assert_eq!(agree_float("1000000.0\n", None), "1000000\n");
    assert_eq!(agree_float("1.0e21\n", None), "1e+21\n");
    assert_eq!(agree_float("1.0e-6\n", None), "0.000001\n");
    assert_eq!(agree_float("1.0e-7\n", None), "1e-7\n");
    assert_eq!(agree_float("0.0001\n", None), "0.0001\n");
    assert_eq!(agree_float("-0.5 * 2.0\n", None), "-1\n");
}
