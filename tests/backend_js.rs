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

// Float is JS-only in this row (kantord/toylang#149): the other backends do not carry it yet,
// so none of these can be a corpus case, which would require every backend to agree. They live
// here, JS-only, until the follow-up rows bring Float to the rest of the backends.

/// A decimal-point literal parses, type-checks, and prints through the JS printer.
#[test]
fn float_literals_parse_and_print() {
    let out = toylang::run_on("1.5 + 0.25 * 2.0\n", None, Backend::Js).unwrap();
    assert_eq!(out, "2\n");
}

/// Division on a Float is total (ADR 0007, the Q37 ruling): a zero divisor is the IEEE answer,
/// Infinity, not the stop an Int's `1 / 0` is.
#[test]
fn float_division_by_zero_is_infinity() {
    let out = toylang::run_on("1.0 / 0.0\n", None, Backend::Js).unwrap();
    assert_eq!(out, "Infinity\n");
}

/// NaN and Infinity are values a Float can hold, and both print as their names.
#[test]
fn float_nan_and_infinity_are_producible() {
    let nan = toylang::run_on("0.0 / 0.0\n", None, Backend::Js).unwrap();
    assert_eq!(nan, "NaN\n");
    let neg = toylang::run_on("-1.0 / 0.0\n", None, Backend::Js).unwrap();
    assert_eq!(neg, "-Infinity\n");
}

/// Float comparisons type-check and print a Bool.
#[test]
fn float_comparison() {
    let out = toylang::run_on("1.5 < 2.5\n", None, Backend::Js).unwrap();
    assert_eq!(out, "true\n");
}

/// A Float result keeps its value when read back off the wire: a JSON number already is the
/// double a Float names (ADR 0007), the case that made Int64 refuse `input`.
#[test]
fn float_input_reads_a_json_number() {
    let src = "fn twice(x: Float) -> Float = x * 2.0\n\ntwice(input)\n";
    let out = toylang::run_on(src, Some("2.5"), Backend::Js).unwrap();
    assert_eq!(out, "5\n");
}
