//! The jq backend against the things it alone can say.
//!
//! Behaviour lives in the corpus like every other backend. What is here is the two rules jq
//! forced that the others did not.

const FORWARD: &str = r#"
fn outer(x: Str) -> Str = inner(x) + "!"
fn inner(x: Str) -> Str = "[" + x + "]"

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
fn a(n: Int) -> Int = 0 if n <= 0 else 1 + b(n - 1)
fn b(n: Int) -> Int = 0 if n <= 0 else 1 + c(n - 1)
fn c(n: Int) -> Int = 0 if n <= 0 else 1 + a(n - 1)

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

/// Direct self-recursion is not a cycle `ordered` ever gets stuck on: a function calling only
/// itself is always immediately ready, so jq keeps running every corpus program that recurses
/// this way (`unlines`, `join`, and every self-recursive corpus case already do).
#[test]
fn self_recursion_alone_still_compiles() {
    let p =
        toylang::compile("fn count(n: Int) -> Int = 0 if n <= 0 else 1 + count(n - 1)\n\ncount(5)")
            .unwrap();
    assert!(toylang::emit_jq::emit(&p).is_ok());
}
