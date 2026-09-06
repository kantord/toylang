//! The Lua backend against the thing it alone can say.
//!
//! Behaviour lives in the corpus like every other backend. What is here is a comparison between
//! two backends, which a corpus entry cannot express because it runs one program and compares
//! outputs.

// Float (kantord/toylang#149) reached the Lua backend with this row, the last of the seven to
// carry it. These are the JS reference cases (tests/backend_js.rs) and the Python ones
// (tests/backend_py.rs) run against Lua: the exact strings are what JS's `String(number)`
// produces, which ADR 0007 names the ground truth every backend matches byte for byte, and
// `tl_show_float` is what re-spells a Lua float into them -- Lua's own `tostring` does not, so the
// helper is the one place the Lua backend diverges from a native `tostring` and re-derives the
// shortest round-trip digits instead.

/// A decimal-point literal parses, type-checks, and prints through the Lua printer.
#[test]
fn float_literals_parse_and_print() {
    let out = toylang::run_on("1.5 + 0.25 * 2.0\n", None, toylang::Backend::Lua).unwrap();
    assert_eq!(out, "2\n");
}

/// Division on a Float is total (ADR 0007): a zero divisor is the IEEE answer, Infinity, which
/// Lua's native `/` produces (`1.0 / 0.0` is `inf`) and `tl_show_float` spells the name of.
#[test]
fn float_division_by_zero_is_infinity() {
    let out = toylang::run_on("1.0 / 0.0\n", None, toylang::Backend::Lua).unwrap();
    assert_eq!(out, "Infinity\n");
}

/// NaN and Infinity are values a Float can hold, and both print as their names. Lua's `0.0 / 0.0`
/// is `-nan`, which the `v ~= v` test catches regardless of the sign the host happens to print.
#[test]
fn float_nan_and_infinity_are_producible() {
    let nan = toylang::run_on("0.0 / 0.0\n", None, toylang::Backend::Lua).unwrap();
    assert_eq!(nan, "NaN\n");
    let neg = toylang::run_on("-1.0 / 0.0\n", None, toylang::Backend::Lua).unwrap();
    assert_eq!(neg, "-Infinity\n");
}

/// Float comparisons type-check and print a Bool, and NaN follows IEEE: not equal to itself, and
/// not less than, not greater than anything either.
#[test]
fn float_comparison() {
    let out = toylang::run_on("1.5 < 2.5\n", None, toylang::Backend::Lua).unwrap();
    assert_eq!(out, "true\n");
    assert_eq!(
        toylang::run_on("(0.0 / 0.0) == (0.0 / 0.0)\n", None, toylang::Backend::Lua).unwrap(),
        "false\n"
    );
    assert_eq!(
        toylang::run_on("(0.0 / 0.0) != (0.0 / 0.0)\n", None, toylang::Backend::Lua).unwrap(),
        "true\n"
    );
}

/// A Float result keeps its value when read back off the wire: a JSON number already is the
/// double a Float names (ADR 0007).
#[test]
fn float_input_reads_a_json_number() {
    let src = "fn twice(x: Float) -> Float = x * 2.0\n\ntwice(parse(stdin))\n";
    let out = toylang::run_on(src, Some("2.5"), toylang::Backend::Lua).unwrap();
    assert_eq!(out, "5\n");
}

/// `-x` on a Float-typed variable, not a literal -- the checker folds `-3.5` straight into a
/// negative literal, so this is the only path that exercises the general `0 - x` desugaring at
/// Float width.
#[test]
fn float_negation_of_a_variable() {
    let src = "fn neg(x: Float) -> Float = -x\n\nneg(3.5)\n";
    let out = toylang::run_on(src, None, toylang::Backend::Lua).unwrap();
    assert_eq!(out, "-3.5\n");
}

/// A record and a Vec of Floats print correctly, exercising `tl_show_float` through the printer's
/// recursive walk (a Vec element, a record field) rather than only at the top level -- the case
/// that `contains_float` on the body type exists to detect, since the helper-injection walk visits
/// TIR nodes by Kind and a Float inside a container is a type fact, not a Kind one.
#[test]
fn float_in_a_record_and_a_vec() {
    assert_eq!(
        toylang::run_on("{a: 1.5, b: 2.5}\n", None, toylang::Backend::Lua).unwrap(),
        "{\"a\":1.5,\"b\":2.5}\n"
    );
    assert_eq!(
        toylang::run_on("[1.5, 2.25, 3.0]\n", None, toylang::Backend::Lua).unwrap(),
        "[1.5,2.25,3]\n"
    );
    assert_eq!(
        toylang::run_on("{a: 1.5, b: 2.5} == {a: 1.5, b: 2.5}\n", None, toylang::Backend::Lua)
            .unwrap(),
        "true\n"
    );
}

/// Printing format across the ECMA-262 `Number::toString` notation-switch boundaries: fixed vs
/// scientific at 1e21 and 1e-7/1e-6, and the trailing zeros an integer-valued Float still needs
/// suppressed. This is where `tl_show_float`'s re-layout does its work -- `tostring` would diverge
/// at every one of these -- so each boundary is pinned to the JS ground truth.
#[test]
fn float_printing_matches_at_notation_boundaries() {
    assert_eq!(toylang::run_on("100.0\n", None, toylang::Backend::Lua).unwrap(), "100\n");
    assert_eq!(
        toylang::run_on("1000000.0\n", None, toylang::Backend::Lua).unwrap(),
        "1000000\n"
    );
    assert_eq!(toylang::run_on("1.0e21\n", None, toylang::Backend::Lua).unwrap(), "1e+21\n");
    assert_eq!(toylang::run_on("1.0e-6\n", None, toylang::Backend::Lua).unwrap(), "0.000001\n");
    assert_eq!(toylang::run_on("1.0e-7\n", None, toylang::Backend::Lua).unwrap(), "1e-7\n");
    assert_eq!(toylang::run_on("0.0001\n", None, toylang::Backend::Lua).unwrap(), "0.0001\n");
    assert_eq!(toylang::run_on("-0.5 * 2.0\n", None, toylang::Backend::Lua).unwrap(), "-1\n");
}

/// A product can push a Float out of the plain-decimal band even when each operand is inside it.
#[test]
fn float_product_above_the_band_goes_exponential() {
    assert_eq!(
        toylang::run_on("1e11 * 1e11\n", None, toylang::Backend::Lua).unwrap(),
        "1e+22\n"
    );
}
