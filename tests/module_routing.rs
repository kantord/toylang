//! `@(path)` module routing (gh:167): a program applies another file's `handle` to `.`. The
//! routed modules are real files under `tests/modules/`, which is why these cases are not corpus
//! cases -- a corpus case is one self-contained `program`. Paths resolve against the directory
//! the program was compiled from, and `toylang::compile` takes text with no file, so they
//! resolve against the working directory: the crate root, which is where cargo runs a test.
//! Every routed program that runs is run on every backend through the corpus's own agreement
//! check, since the merged module functions are ordinary functions to each backend.

mod support;

use support::{Expect, agreement_failures};

fn refusal(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// Every backend runs `program`, agrees, and prints `want`.
fn agrees(name: &str, program: &str, want: &str) {
    let failures = agreement_failures(name, program, None, &Expect::Output(want.to_string()));
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn routed_call_applies_the_module_handle_to_the_subject() {
    agrees("double", r#"21 | @("tests/modules/double.toy")"#, "42\n");
}

/// The router shape the feature was asked for (gh:162): each matcher arm dispatches its payload
/// to a different file.
#[test]
fn matcher_arms_route_to_different_modules() {
    agrees(
        "router",
        r#"enum Temp { Celsius(Int), Kelvin(Int) }

fn to_kelvin(t: Temp) -> Int =
    t | Celsius -> @("tests/modules/celsius.toy") or Kelvin -> @("tests/modules/kelvin.toy")

[to_kelvin(Temp.celsius(27)), to_kelvin(Temp.kelvin(300))]
"#,
        "[300,300]\n",
    );
}

/// A module's `handle` may call the module's own private helper: the definition is checked as
/// that file, the same origin rule that lets a `pub` prelude function use a private one.
#[test]
fn module_handle_calls_its_own_private_helper() {
    agrees(
        "greet",
        r#""bob" | @("tests/modules/greet.toy")"#,
        "hello bob!\n",
    );
}

#[test]
fn program_cannot_call_a_module_private_helper() {
    insta::assert_snapshot!(refusal(
        r#""bob" | @("tests/modules/greet.toy") | exclaim(.)"#
    ));
}

/// Full merge, prelude-style: a routed module's `pub` definitions are callable from the program
/// by their bare names, exactly as the prelude's are.
#[test]
fn module_pub_definitions_merge_by_name() {
    agrees(
        "shout",
        r#""bob" | @("tests/modules/greet.toy") | shout(.)"#,
        "hello bob!!!\n",
    );
}

/// A module's enum is merged, but its variants are only reachable qualified: bare `circle{r: 2}`
/// never resolves through the variant lookup, so two modules sharing a variant name are never a
/// collision the program has to resolve.
#[test]
fn module_enum_variants_are_always_qualified() {
    agrees(
        "qualified",
        r#"Shape.circle{r: 2} | @("tests/modules/shapes.toy")"#,
        "12\n",
    );
    insta::assert_snapshot!(refusal(r#"circle{r: 2} | @("tests/modules/shapes.toy")"#));
}

#[test]
fn wrong_subject_type_is_refused() {
    insta::assert_snapshot!(refusal(r#""x" | @("tests/modules/double.toy")"#));
}

#[test]
fn route_with_no_subject_is_refused() {
    insta::assert_snapshot!(refusal(r#"@("tests/modules/double.toy")"#));
}

#[test]
fn module_without_handle_is_refused() {
    insta::assert_snapshot!(refusal(r#"1 | @("tests/modules/no_handle.toy")"#));
}

#[test]
fn unreadable_module_is_refused() {
    insta::assert_snapshot!(refusal(r#"1 | @("tests/modules/missing.toy")"#));
}

/// A routed module's own `@(path)` resolves against that module's directory, not the program's.
#[test]
fn module_routes_resolve_relative_to_the_module() {
    agrees(
        "nested",
        r#"41 | @("tests/modules/nested/outer.toy")"#,
        "42\n",
    );
}

/// The entry's internal name carries the path, so two modules' `handle`s coexist in one program
/// and every backend renders it as a legal identifier.
#[test]
fn emitted_entry_name_carries_the_module_path() {
    let program = toylang::compile(r#"21 | @("tests/modules/double.toy")"#).unwrap();
    insta::assert_snapshot!(toylang::Backend::Lua.emit(&program).unwrap());
}

#[test]
fn formatter_keeps_the_route_spelling() {
    let src = "21 | @(\"tests/modules/double.toy\")\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
}
