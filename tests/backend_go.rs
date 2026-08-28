//! The Go backend against the things it alone can say.
//!
//! Behaviour lives in the corpus like every other backend. What is here is what falls out of Go
//! being the first target that is statically typed with no runtime type information: it needs a
//! declared name for every record, and it rejects an import nothing uses.

/// A record type here is structural, so every spelling of `{name: Str, age: Int}` is one type
/// however often it is written. Go is nominal and needs a declaration, so this is the first
/// backend that has to decide how many types the program actually has -- and getting it wrong
/// would mean two Go structs that no assignment between them would typecheck.
#[test]
fn one_struct_per_record_type() {
    let src = r#"
fn keep(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<{name: Str, age: Int}> = db.users
fn name(u: {name: Str, age: Int}) -> Str = u.name

keep(input)
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
