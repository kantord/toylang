//! Type aliases.
//!
//! An alias is an abbreviation and nothing else, so the tests that matter are the ones showing it
//! leaves no trace: not in the emitted code, not in an error message, not anywhere a reader could
//! tell one was used.

use toylang::Backend;

const WITH_ALIAS: &str = r#"
type U = {name: Str, age: Int}
type Db = {users: Vec<U>}

fn adults(db: Db) -> Vec<Str> = db.users | select(.age >= 18) | .[].name

adults(input)
"#;

const WRITTEN_OUT: &str = r#"
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users | select(.age >= 18) | .[].name

adults(input)
"#;

/// The strongest statement of transparency available: every backend emits the same bytes either
/// way, so the alias is gone before any of them sees the program.
#[test]
fn an_alias_emits_identically_to_the_type_written_out() {
    let aliased = toylang::compile(WITH_ALIAS).unwrap();
    let plain = toylang::compile(WRITTEN_OUT).unwrap();
    let mut checked = 0;
    for backend in Backend::ALL {
        assert_eq!(
            backend.emit(&aliased).unwrap(),
            backend.emit(&plain).unwrap(),
            "{} emits differently for an alias",
            backend.name()
        );
        checked += 1;
    }
    assert_eq!(checked, Backend::ALL.len(), "every backend has to have been tried");
}

/// An alias has no identity, so an error reports the shape rather than the name. This is the
/// half of Q34 that is settled: naming a type does not make it a different type.
#[test]
fn an_alias_is_invisible_in_errors() {
    insta::assert_snapshot!(
        toylang::compile("type Db = {users: Vec<Int>}\n\nfn f(d: Db) -> Str = d\n\nf(input)")
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// Expanding this would not terminate, so it is refused rather than attempted.
#[test]
fn a_type_written_in_terms_of_itself() {
    insta::assert_snapshot!(err("type T = {next: T}\n\nstr(1)"));
}

/// The cycle need not be direct, and the chain of names being expanded is what catches it.
#[test]
fn a_cycle_through_two_names() {
    insta::assert_snapshot!(err("type A = {b: B}\ntype B = {a: A}\n\nstr(1)"));
}

/// Resolved eagerly, so a broken alias is an error even when nothing refers to it.
#[test]
fn an_unused_alias_is_still_checked() {
    insta::assert_snapshot!(err("type A = Nope\n\nstr(1)"));
}

/// Naming the chain is the difference between knowing there is a cycle and finding it.
#[test]
fn a_cycle_through_three_names() {
    insta::assert_snapshot!(err("type A = {b: B}\ntype B = {c: C}\ntype C = {a: A}\n\nstr(1)"));
}

#[test]
fn a_builtin_type_cannot_be_redefined() {
    insta::assert_snapshot!(err("type Int = Str\n\nstr(1)"));
}

/// The casing rule reaches type declarations too, which is what will keep a constructor and a
/// call apart if named types ever gain identity.
#[test]
fn a_type_name_starts_uppercase() {
    insta::assert_snapshot!(err("type db = {users: Vec<Int>}\n\nstr(1)"));
}

#[test]
fn a_type_cannot_be_defined_twice() {
    insta::assert_snapshot!(err("type A = Int\ntype A = Str\n\nstr(1)"));
}
