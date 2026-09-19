//! The jq backend against the things it alone can say.
//!
//! Behaviour lives in the corpus like every other backend. What is here is the two rules jq
//! forced that the others did not.

const FORWARD: &str = r#"
fn outer(x: Str) -> Str = inner(x) + "!";
fn inner(x: Str) -> Str = "[" + x + "]";

outer("hi")
"#;

/// jq resolves a `def` only against what is already defined and has no forward declaration, so
/// definitions come out callee-first. The checker accepts the other order, which is a rule this
/// target does not share.
#[test]
fn definitions_come_out_callee_first() {
    let p = toylang::compile(FORWARD).unwrap();
    let jq = toylang::emit_jq::emit(&p).unwrap();
    let inner = jq.find("def v_inner").expect("inner is defined");
    let outer = jq.find("def v_outer").expect("outer is defined");
    assert!(inner < outer, "callee must be defined first:\n{jq}");
    insta::assert_snapshot!(jq);
}

/// jq's -r decides from the runtime value, so it would print a present Opt<Str> raw and an
/// absent one as the word null. The rule here is the type's, as on every other backend.
#[test]
fn an_optional_string_prints_as_json() {
    insta::assert_snapshot!(
        toylang::run_on(r#"["ada", "bo"][0]"#, None, toylang::Backend::Jq).unwrap()
    );
}

/// `a` calls `b` calls `c` calls `a`: a real cycle between three named functions, the shape
/// `plans/mini-parser-spike.md` found in a recursive-descent parser's own
/// `expr`/`term`/`factor`/`group` chain (kantord/toylang#77, kantord/toylang#79). The checker
/// accepts it -- signatures are collected before any body is checked, so a call to a function
/// defined later, or back around a cycle, is no different from any forward reference.
const CYCLE: &str = r#"
fn a(n: Int) -> Int = n | . <= 0 -> 0 or 1 + b(n - 1);
fn b(n: Int) -> Int = n | . <= 0 -> 0 or 1 + c(n - 1);
fn c(n: Int) -> Int = n | . <= 0 -> 0 or 1 + a(n - 1);

a(5)
"#;

/// The six backends this cycle does not defeat: jq's `def` scoping is the one thing about this
/// program that is backend-specific, so it cannot live in the corpus (kantord/toylang#79's own
/// AGENTS.md rule -- every corpus case runs on every backend, and jq never can here). This pins
/// the same "every backend agrees" claim by hand, over `Backend::ALL` minus `Jq`.
#[test]
fn mutual_recursion_runs_and_agrees_on_every_backend_but_jq() {
    let mut outputs: Vec<(&str, String)> = Vec::new();
    for backend in toylang::Backend::ALL {
        if backend == toylang::Backend::Jq {
            continue;
        }
        match toylang::run_on(CYCLE, None, backend) {
            Ok(out) => outputs.push((backend.name(), out)),
            Err(e) => panic!("{} could not run the cycle: {e}", backend.name()),
        }
    }
    let (_, first) = &outputs[0];
    assert_eq!(first, "5\n");
    for (name, out) in &outputs {
        assert_eq!(out, first, "{name} disagreed with {}", outputs[0].0);
    }
}

/// The cycle jq alone cannot take: `ordered` cannot find any definition order where every
/// function's callees are already in scope, so it refuses rather than emitting jq source that
/// would fail to compile with an error naming a mangled internal name out of context
/// (kantord/toylang#79).
#[test]
fn a_genuine_cycle_between_named_functions_is_refused_cleanly() {
    let p = toylang::compile(CYCLE).unwrap();
    let err = toylang::emit_jq::emit(&p).unwrap_err();
    assert!(
        err.contains('a') && err.contains('b') && err.contains('c'),
        "{err}"
    );
    insta::assert_snapshot!(err);
}

/// Two enums that reach each other only through `Vec` type-check legally (kantord/toylang#94),
/// but their printers form the same genuine cycle named functions can: `tl_show_A` calls
/// `tl_show_B` calls `tl_show_A`, and jq's `def` has no forward declaration for that any more
/// than it does for `a`/`b`/`c` above (kantord/toylang#116).
const PRINTER_CYCLE: &str = r#"
enum A { A(Vec<B>) }
enum B { B(Vec<A>) }

{x: A.a([]), y: B.b([])}
"#;

/// The printer-side counterpart to `a_genuine_cycle_between_named_functions_is_refused_cleanly`:
/// before kantord/toylang#116, `printers()` emitted defs in DFS-discovery order with no cycle
/// check, so this program would have failed with jq's own raw error naming a mangled internal
/// name instead of toylang's clean refusal.
#[test]
fn a_genuine_cycle_between_printers_is_refused_cleanly() {
    let p = toylang::compile(PRINTER_CYCLE).unwrap();
    let err = toylang::emit_jq::emit(&p).unwrap_err();
    assert!(
        err.contains("tl_show_A") && err.contains("tl_show_B"),
        "{err}"
    );
    insta::assert_snapshot!(err);
}

/// Direct self-recursion is not a cycle `ordered` ever gets stuck on: a function calling only
/// itself is always immediately ready, so jq keeps running every corpus program that recurses
/// this way (`join_lines`, `join`,and every self-recursive corpus case already do).
#[test]
fn self_recursion_alone_still_compiles() {
    let p = toylang::compile(
        "fn count(n: Int) -> Int = n | . <= 0 -> 0 or 1 + count(n - 1);\n\ncount(5)",
    )
    .unwrap();
    assert!(toylang::emit_jq::emit(&p).is_ok());
}

/// Float is JS-only and jq-only in this row (kantord/toylang#149): the other backends do not
/// carry it yet, so none of these can be a corpus case, which would require every backend to
/// agree. They pin jq's output byte for byte against what the JS reference prints, so the two
/// backends that carry Float stay honest with each other without roping in the rest of the row.
/// Nested floats used to be the one place they deliberately did not agree; since the text
/// renderer (`text` in src/emit_jq.rs) they agree there too, see below.
#[track_caller]
fn agree_jq_js(src: &str, stdin: Option<&str>) -> String {
    let js = toylang::run_on(src, stdin, toylang::Backend::Js).unwrap();
    let jq = toylang::run_on(src, stdin, toylang::Backend::Jq).unwrap();
    assert_eq!(js, jq, "js and jq disagree on:\n{src}");
    js
}

/// A decimal-point literal parses, type-checks, and prints as a bare number the way JS prints it.
#[test]
fn float_literals_parse_and_print() {
    assert_eq!(agree_jq_js("1.5 + 0.25 * 2.0\n", None), "2\n");
    assert_eq!(agree_jq_js("2.0\n", None), "2\n");
}

/// Division on a Float is total (ADR 0007, the Q37 ruling): a zero divisor is the IEEE answer,
/// Infinity, not the stop an Int's `1 / 0` is. jq's own `/` rejects a zero divisor, so this
/// pins the `tl_fdiv` bridge instead.
#[test]
fn float_division_by_zero_is_infinity() {
    assert_eq!(agree_jq_js("1.0 / 0.0\n", None), "Infinity\n");
}

/// NaN and Infinity are values a Float can hold, and both print as their names, the same bare
/// words the JS backend prints (which is why a Float body runs with `-r` in `run_jq`).
#[test]
fn float_nan_and_infinity_are_producible() {
    assert_eq!(agree_jq_js("0.0 / 0.0\n", None), "NaN\n");
    assert_eq!(agree_jq_js("-1.0 / 0.0\n", None), "-Infinity\n");
}

/// Float comparisons type-check and print a Bool.
#[test]
fn float_comparison() {
    assert_eq!(agree_jq_js("1.5 < 2.5\n", None), "true\n");
    assert_eq!(agree_jq_js("(0.0 / 0.0) == (0.0 / 0.0)\n", None), "false\n");
    assert_eq!(agree_jq_js("(0.0 / 0.0) != (0.0 / 0.0)\n", None), "true\n");
}

/// A Float result keeps its value when read back off the wire: a JSON number already is the
/// double a Float names (ADR 0007), the case that made Int64 refuse `input`.
#[test]
fn float_input_reads_a_json_number() {
    let src = "fn twice(x: Float) -> Float = x * 2.0;\n\ntwice(parse(stdin))\n";
    assert_eq!(agree_jq_js(src, Some("2.5")), "5\n");
}

/// Finite floats nested inside a Vec or Record print as JSON numbers, matching the JS reference,
/// including at the notation-switch boundaries below (see `float_printing_matches_at_notation_boundaries`).
#[test]
fn float_inside_a_vec_and_a_record() {
    assert_eq!(agree_jq_js("[1.5, 2.5]\n", None), "[1.5,2.5]\n");
    assert_eq!(
        agree_jq_js("{a: 1.5, b: 2.5}\n", None),
        "{\"a\":1.5,\"b\":2.5}\n"
    );
}

/// The printer's notation-switch rewrite (`tl_show_float` in src/emit_jq.rs): jq's own
/// `tostring` gets the shortest round-trip digits right (verified in isolation against a
/// ~5000-value fuzz run before this test was written) but switches to scientific notation at a
/// different, and not entirely consistent, magnitude than JS's ECMA-262 rule, and pads its
/// exponent to two digits. These are the values that would fail without the rewrite.
#[test]
fn float_printing_matches_at_notation_boundaries() {
    assert_eq!(agree_jq_js("100.0\n", None), "100\n");
    assert_eq!(agree_jq_js("1000000.0\n", None), "1000000\n");
    assert_eq!(agree_jq_js("1.0e21\n", None), "1e+21\n");
    assert_eq!(agree_jq_js("1.0e-6\n", None), "0.000001\n");
    assert_eq!(agree_jq_js("1.0e-7\n", None), "1e-7\n");
    assert_eq!(agree_jq_js("0.0001\n", None), "0.0001\n");
    assert_eq!(agree_jq_js("-0.5 * 2.0\n", None), "-1\n");
}

/// What jq's own encoder cannot reproduce, and the text renderer does: a Float inside a Vec,
/// a record, an enum payload, or a `jsonlines` element. jq's `-c` output prints a nested
/// double in its own notation (`1E-7`, a 22-digit run for `1e21`) and turns the non-finite
/// values into the largest double or `null`, so emit_jq.rs assembles any structure holding a
/// Float as JSON text around `tl_show_float` instead (`text`), and `run_jq` prints it raw.
/// Each shape is pinned against the JS reference; the recursive enum is the one that goes
/// through a named `_text` printer rather than an inline expansion.
#[test]
fn float_inside_a_container_agrees_with_js_at_every_position() {
    assert_eq!(agree_jq_js("[1.0 / 0.0, 0.0 / 0.0]\n", None), "[Infinity,NaN]\n");
    assert_eq!(
        agree_jq_js("{a: 1e21, b: 1e-7, s: \"x\"}\n", None),
        "{\"a\":1e+21,\"b\":1e-7,\"s\":\"x\"}\n"
    );
    assert_eq!(agree_jq_js("[1.5][5]\n", None), "null\n");
    assert_eq!(agree_jq_js("[1.5][0]\n", None), "1.5\n");
    assert_eq!(
        agree_jq_js(
            "enum Shape { Point, Circle{r: Float} }\n\n[circle({r: 1e21}), Shape.point]\n",
            None
        ),
        "[{\"Circle\":{\"r\":1e+21}},\"Point\"]\n"
    );
    assert_eq!(
        agree_jq_js(
            "enum Tree { Leaf{v: Float}, Node{kids: Vec<Tree>} }\n\n\
             node({kids: [leaf({v: 1e21}), leaf({v: 0.0 / 0.0})]})\n",
            None
        ),
        "{\"Node\":{\"kids\":[{\"Leaf\":{\"v\":1e+21}},{\"Leaf\":{\"v\":NaN}}]}}\n"
    );
    assert_eq!(
        agree_jq_js("jsonlines([1e21, 0.0 / 0.0])\n", None),
        "1e+21\nNaN\n"
    );
}
