//! The Go backend against the things it alone can say.
//!
//! Behaviour lives in the corpus like every other backend. What is here is what falls out of Go
//! being the first target that is statically typed with no runtime type information: it needs a
//! declared name for every record, and it rejects an import nothing uses.

use toylang::Backend;

/// A record type here is structural, so every spelling of `{name: Str, age: Int}` is one type
/// however often it is written. Go is nominal and needs a declaration, so this is the first
/// backend that has to decide how many types the program actually has -- and getting it wrong
/// would mean two Go structs that no assignment between them would typecheck.
#[test]
fn one_struct_per_record_type() {
    let src = r#"
fn keep(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<{name: Str, age: Int}> = db.users
fn name(u: {name: Str, age: Int}) -> Str = u.name

keep(parse(stdin))
"#;
    let p = toylang::compile(src).unwrap();
    let go = toylang::emit_go::emit(&p);
    // The user record and the wrapper around it, and nothing more: the three occurrences of
    // the user record are one type.
    assert_eq!(
        go.matches("type tlRec").count(),
        2,
        "one struct per record type:\n{go}"
    );
}

/// An unused import does not compile in Go, so the import list cannot be padded the way an
/// unused helper can. This is why the imports come from walking the program while the helpers
/// are read back off the emitted text.
#[test]
fn imports_are_exactly_what_is_used() {
    let strings_only = toylang::emit_go::emit(&toylang::compile(r#""a" + "b""#).unwrap());
    assert!(
        !strings_only.contains("strconv"),
        "nothing here formats a number:\n{strings_only}"
    );
    assert!(
        !strings_only.contains("encoding/json"),
        "nothing here reads input:\n{strings_only}"
    );

    let with_int = toylang::emit_go::emit(&toylang::compile("[1, 2]").unwrap());
    assert!(
        with_int.contains("\"strconv\""),
        "printing an Int needs strconv:\n{with_int}"
    );
}

// Float is landed for Go and JS, and until the rest of the backends carry it none of these can
// be a corpus case, which would require every backend to agree. The expected strings are JS's
// `String(number)` (ECMA-262), which is what the corpus's agreement harness compares against.

/// A decimal-point literal parses, type-checks, and prints through the Go printer.
#[test]
fn float_literals_parse_and_print() {
    let out = toylang::run_on("1.5 + 0.25 * 2.0\n", None, Backend::Go).unwrap();
    assert_eq!(out, "2\n");
}

/// Division on a Float is total (ADR 0007, the Q37 ruling): a zero divisor is the IEEE answer,
/// Infinity, not the stop an Int's `1 / 0` is.
#[test]
fn float_division_by_zero_is_infinity() {
    let out = toylang::run_on("1.0 / 0.0\n", None, Backend::Go).unwrap();
    assert_eq!(out, "Infinity\n");
}

/// NaN and Infinity are values a Float can hold, and both print as their names.
#[test]
fn float_nan_and_infinity_are_producible() {
    let nan = toylang::run_on("0.0 / 0.0\n", None, Backend::Go).unwrap();
    assert_eq!(nan, "NaN\n");
    let neg = toylang::run_on("-1.0 / 0.0\n", None, Backend::Go).unwrap();
    assert_eq!(neg, "-Infinity\n");
}

/// Float comparisons type-check and print a Bool.
#[test]
fn float_comparison() {
    let out = toylang::run_on("1.5 < 2.5\n", None, Backend::Go).unwrap();
    assert_eq!(out, "true\n");
}

/// A Float result keeps its value when read back off the wire: a JSON number already is the
/// double a Float names (ADR 0007), the case that made Int64 refuse `input`.
#[test]
fn float_input_reads_a_json_number() {
    let src = "fn twice(x: Float) -> Float = x * 2.0\n\ntwice(parse(stdin))\n";
    let out = toylang::run_on(src, Some("2.5"), Backend::Go).unwrap();
    assert_eq!(out, "5\n");
}

/// The printer matches JS's `String(number)` at the notation-switch boundaries, where Go's
/// default formatting would diverge: fixed notation for 1e-6 up to 1e21, scientific outside it,
/// and no trailing `.0` on a whole value.
#[test]
fn float_notation_switches_like_js() {
    for (src, want) in [
        ("100000.0\n", "100000\n"),
        ("1e20\n", "100000000000000000000\n"),
        ("1e21\n", "1e+21\n"),
        ("0.000001\n", "0.000001\n"),
        ("1e-6\n", "0.000001\n"),
        ("1e-7\n", "1e-7\n"),
        ("1.5\n", "1.5\n"),
        ("-0.0\n", "0\n"),
        ("2.5e-5\n", "0.000025\n"),
        ("1.234e-6\n", "0.000001234\n"),
    ] {
        assert_eq!(
            toylang::run_on(src, None, Backend::Go).unwrap(),
            want,
            "on {src:?}"
        );
    }
}
