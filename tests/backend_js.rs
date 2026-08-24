//! What the corpus cannot express. Behaviour that must be identical across backends lives in
//! tests/corpus/ and is checked by tests/corpus.rs; this file holds the claims that need more
//! than one run of one program to state.

use toylang::Backend;

#[track_caller]
fn agree(src: &str, stdin: Option<&str>) -> String {
    let mut results =
        Backend::ALL.iter().map(|b| (b.name(), toylang::run_on(src, stdin, *b).unwrap()));
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
