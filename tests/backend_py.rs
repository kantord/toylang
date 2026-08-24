//! The Python backend against the thing it alone can say.
//!
//! Behaviour lives in the corpus like every other backend. What is here is a comparison between
//! two backends, which a corpus entry cannot express because it runs one program and compares
//! outputs.

const ADULTS: &str = r#"
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users | select(.age >= 18) | .[].name

adults(input)
"#;

#[test]
fn emitted_py() {
    let p = toylang::compile(ADULTS).unwrap();
    insta::assert_snapshot!(toylang::emit_py::emit(&p));
}

/// A record is a dict, which is what `json.loads` already returns, so reading input is the parse
/// and nothing else. Go reaches the same value through two declared structs and a decoder.
///
/// This is the clearest reading available on how much a target has to be told about the type
/// model: the same program, the same types, and one backend needs no type declarations at all
/// while the other cannot proceed without them.
#[test]
fn reading_input_costs_python_nothing_and_go_two_declarations() {
    let p = toylang::compile(ADULTS).unwrap();

    let py = toylang::emit_py::emit(&p);
    assert_eq!(py.matches("json.").count(), 1, "one parse, no decoding:\n{py}");

    let go = toylang::emit_go::emit(&p);
    assert_eq!(go.matches("type tlRec").count(), 2, "the same two record types:\n{go}");
}
