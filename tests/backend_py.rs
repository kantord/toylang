//! The Python backend against the thing it alone can say.
//!
//! Behaviour lives in the corpus like every other backend. What is here is a comparison between
//! two backends, which a corpus entry cannot express because it runs one program and compares
//! outputs.

mod support;

/// The corpus holds the one copy of this program; three test files each had their own.
fn adults() -> String {
    support::cases()
        .into_iter()
        .find(|c| c.name == "adults")
        .expect("the corpus has an `adults` case")
        .program
}

/// A record is a dict, which is what `json.loads` already returns, so reading input is the parse
/// and nothing else. Go reaches the same value through two declared structs and a decoder.
///
/// This is the clearest reading available on how much a target has to be told about the type
/// model: the same program, the same types, and one backend needs no type declarations at all
/// while the other cannot proceed without them.
#[test]
fn reading_input_costs_python_nothing_and_go_two_declarations() {
    let p = toylang::compile(&adults()).unwrap();

    let py = toylang::emit_py::emit(&p);
    assert_eq!(
        py.matches("json.").count(),
        1,
        "one parse, no decoding:\n{py}"
    );

    let go = toylang::emit_go::emit(&p);
    assert_eq!(
        go.matches("type tlRec").count(),
        2,
        "the same two record types:\n{go}"
    );
}

// Float (kantord/toylang#149) only reached the Python backend with the follow-up row, so these
// are the JS reference cases (tests/backend_js.rs) run against Py, plus the exponent-band
// boundaries the JS file did not need to reach. Python's `repr` and JS's `String(number)`
// pick the same shortest digits, so the only difference is the framing; tl_float reshapes it.

/// A decimal-point literal parses, type-checks, and prints through the Python printer.
#[test]
fn float_literals_parse_and_print() {
    let out = toylang::run_on("1.5 + 0.25 * 2.0\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(out, "2\n");
}

/// Division on a Float is total (ADR 0007): a zero divisor is the IEEE answer, Infinity.
#[test]
fn float_division_by_zero_is_infinity() {
    let out = toylang::run_on("1.0 / 0.0\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(out, "Infinity\n");
}

/// NaN and Infinity are values a Float can hold, and both print as their names.
#[test]
fn float_nan_and_infinity_are_producible() {
    let nan = toylang::run_on("0.0 / 0.0\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(nan, "NaN\n");
    let neg = toylang::run_on("-1.0 / 0.0\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(neg, "-Infinity\n");
}

/// Float comparisons type-check and print a Bool.
#[test]
fn float_comparison() {
    let out = toylang::run_on("1.5 < 2.5\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(out, "true\n");
}

/// A Float result keeps its value when read back off the wire: a JSON number already is the
/// double a Float names (ADR 0007).
#[test]
fn float_input_reads_a_json_number() {
    let src = "fn twice(x: Float) -> Float = x * 2.0\n\ntwice(parse(stdin))\n";
    let out = toylang::run_on(src, Some("2.5"), toylang::Backend::Py).unwrap();
    assert_eq!(out, "5\n");
}

/// JS keeps a plain decimal for exponents -6..20 and switches to exponential outside it; the
/// Python emitter reshapes repr to that same band, so the boundaries read identically.
#[test]
fn float_upper_band_edge_stays_longhand() {
    let out = toylang::run_on("1e20\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(out, "100000000000000000000\n");
}

#[test]
fn float_above_the_band_goes_exponential() {
    let out = toylang::run_on("1e21\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(out, "1e+21\n");
}

#[test]
fn float_lower_band_edge_stays_longhand() {
    let out = toylang::run_on("1e-6\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(out, "0.000001\n");
}

#[test]
fn float_below_the_band_goes_exponential() {
    let out = toylang::run_on("1e-7\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(out, "1e-7\n");
}

/// A product can push a Float out of the plain-decimal band even when each operand is inside
/// it. Rust's `Display` emits a whole-number float as a bare integer, which Python reads as an
/// int; without the tl_float coercion an exact-int product would spell all 22 digits instead of
/// the exponential JS prints.
#[test]
fn float_product_above_the_band_goes_exponential() {
    let out = toylang::run_on("1e11 * 1e11\n", None, toylang::Backend::Py).unwrap();
    assert_eq!(out, "1e+22\n");
}
