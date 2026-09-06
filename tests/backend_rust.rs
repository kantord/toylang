//! The Rust backend against the corpus.
//!
//! Rust joined `Backend::ALL` once it could compile the whole corpus, the same way native did.
//! `tests/corpus.rs` now covers it like any other backend; this file is what watched the gap
//! while it was still open, and stays as a snapshot of the "not yet" list -- empty for now, and
//! it should only ever grow back temporarily, never silently.

mod support;

use toylang::Backend;

#[test]
fn rust_agrees_where_it_compiles() {
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();

    for case in support::cases() {
        let (name, src, input) = (case.name, case.program, case.input);
        let program = toylang::compile(&src).expect("corpus programs compile");
        let source = toylang::emit_rs::emit(&program);

        let dir = tempfile::tempdir().expect("temp dir");
        let exe = dir.path().join("program");
        match toylang::link_rust(&source, &exe) {
            Err(reason) => unsupported.push(format!("{name}: {reason}")),
            Ok(()) => {
                // Compared as results rather than as output, so a case that every backend has
                // to refuse is checked here too: both refusing is agreement, and one refusing
                // while the other runs is the disagreement worth catching.
                let rust = toylang::run_on(&src, input.as_deref(), Backend::Rust);
                let lua = toylang::run_on(&src, input.as_deref(), Backend::Lua);
                match (rust, lua) {
                    (Ok(r), Ok(l)) => assert_eq!(r, l, "{name}: rust and lua disagree"),
                    (Err(_), Err(_)) => {}
                    (r, l) => panic!("{name}: rust gave {r:?} and lua gave {l:?}"),
                }
                supported.push(name);
            }
        }
    }

    assert!(
        !supported.is_empty(),
        "rust compiles nothing, so this test proves nothing"
    );

    insta::assert_snapshot!(format!(
        "compiles (rust) ({}):\n{}\n\nnot yet ({}):\n{}",
        supported.len(),
        supported.join("\n"),
        unsupported.len(),
        unsupported.join("\n")
    ));
}

/// Float is JS-only and Rust-only in this row (kantord/toylang#149): the other backends do not
/// carry it yet, so none of these can be a corpus case, which would require every backend to
/// agree. They pin Rust's output byte for byte against what the JS reference prints, so the two
/// backends that carry Float stay honest with each other without roping in the rest of the row.
/// Rust's `f64` arithmetic is IEEE already and its printer rebuilds JS's `String(number)`
/// spelling, so unlike the jq lane there is no nested-non-finite divergence to carve out.
#[track_caller]
fn agree_rust_js(src: &str, stdin: Option<&str>) -> String {
    let js = toylang::run_on(src, stdin, toylang::Backend::Js).unwrap();
    let rust = toylang::run_on(src, stdin, toylang::Backend::Rust).unwrap();
    assert_eq!(js, rust, "js and rust disagree on:\n{src}");
    js
}

/// A decimal-point literal parses, type-checks, and prints as a bare number the way JS prints it.
#[test]
fn float_literals_parse_and_print() {
    assert_eq!(agree_rust_js("1.5 + 0.25 * 2.0\n", None), "2\n");
    assert_eq!(agree_rust_js("2.0\n", None), "2\n");
}

/// Division on a Float is total (ADR 0007, the Q37 ruling): a zero divisor is the IEEE answer,
/// Infinity, not the stop an Int's `1 / 0` is. Rust's own `f64` `/` gives the IEEE answers, so
/// no div helper is emitted the way the integer widths need.
#[test]
fn float_division_by_zero_is_infinity() {
    assert_eq!(agree_rust_js("1.0 / 0.0\n", None), "Infinity\n");
}

/// NaN and Infinity are values a Float can hold, and both print as their names, the same bare
/// words the JS backend prints.
#[test]
fn float_nan_and_infinity_are_producible() {
    assert_eq!(agree_rust_js("0.0 / 0.0\n", None), "NaN\n");
    assert_eq!(agree_rust_js("-1.0 / 0.0\n", None), "-Infinity\n");
}

/// Float comparisons type-check and print a Bool.
#[test]
fn float_comparison() {
    assert_eq!(agree_rust_js("1.5 < 2.5\n", None), "true\n");
    assert_eq!(agree_rust_js("(0.0 / 0.0) == (0.0 / 0.0)\n", None), "false\n");
    assert_eq!(agree_rust_js("(0.0 / 0.0) != (0.0 / 0.0)\n", None), "true\n");
}

/// A Float result keeps its value when read back off the wire: a JSON number already is the
/// double a Float names (ADR 0007), the case that made Int64 refuse `input`.
#[test]
fn float_input_reads_a_json_number() {
    let src = "fn twice(x: Float) -> Float = x * 2.0\n\ntwice(input)\n";
    assert_eq!(agree_rust_js(src, Some("2.5")), "5\n");
}

/// Finite floats nested inside a Vec or Record print as JSON numbers, matching the JS reference.
#[test]
fn float_inside_a_vec_and_a_record() {
    assert_eq!(agree_rust_js("[1.5, 2.5]\n", None), "[1.5,2.5]\n");
    assert_eq!(
        agree_rust_js("{a: 1.5, b: 2.5}\n", None),
        "{\"a\":1.5,\"b\":2.5}\n"
    );
}

/// The printer's notation-switch rewrite (`tl_float_to_str` in src/emit_rs.rs): Rust's own
/// `Display` holds fixed notation across the whole range where JS goes scientific outside the
/// ECMA-262 decimal band, so these are the values that would fail without the rewrite.
#[test]
fn float_printing_matches_at_notation_boundaries() {
    assert_eq!(agree_rust_js("100.0\n", None), "100\n");
    assert_eq!(agree_rust_js("1000000.0\n", None), "1000000\n");
    assert_eq!(agree_rust_js("1.0e21\n", None), "1e+21\n");
    assert_eq!(agree_rust_js("1.0e-6\n", None), "0.000001\n");
    assert_eq!(agree_rust_js("1.0e-7\n", None), "1e-7\n");
    assert_eq!(agree_rust_js("0.0001\n", None), "0.0001\n");
    assert_eq!(agree_rust_js("-0.5 * 2.0\n", None), "-1\n");
}

/// A non-finite float nested inside a Vec or Record prints as a bare word at every position,
/// matching the JS reference: Rust's printer runs the same way top-level or nested, so `1.0 /
/// 0.0` is Infinity even inside `[...]`. Unlike jq there is no divergence to pin here.
#[test]
fn float_non_finite_inside_a_vec_matches_js() {
    assert_eq!(
        agree_rust_js("[1.0 / 0.0, 0.0 / 0.0]\n", None),
        "[Infinity,NaN]\n"
    );
}
