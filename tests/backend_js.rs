use toylang::Backend;

const DB: &str = r#"{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": 9}]}"#;

const ADULTS: &str = r#"
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users[] | select(.age >= 18) | .name

adults(input)
"#;

/// Every backend must produce the same output for the same program. This is the check a single
/// backend cannot express, and step 3 makes it systematic over a corpus.
#[track_caller]
fn agree(src: &str, stdin: Option<&str>) -> String {
    let mut results = Backend::ALL
        .iter()
        .map(|b| (b.name(), toylang::run_on(src, stdin, *b).unwrap()));
    let (first_name, first) = results.next().expect("at least one backend");
    for (name, out) in results {
        assert_eq!(first, out, "{first_name} and {name} disagree on:\n{src}");
    }
    first
}

#[test]
fn hello_world() {
    insta::assert_snapshot!(agree(r#""hello world""#, None));
}

#[test]
fn concat() {
    insta::assert_snapshot!(agree(r#""hello " + "world""#, None));
}

#[test]
fn functions() {
    let src = r#"
fn outer(x: Str) -> Str = inner(x) + "!"
fn inner(x: Str) -> Str = "[" + x + "]"

outer("hi")
"#;
    insta::assert_snapshot!(agree(src, None));
}

#[test]
fn filter_a_vec() {
    insta::assert_snapshot!(agree("[1, 2, 3][] | select(. >= 2)", None));
}

#[test]
fn filter_strings() {
    insta::assert_snapshot!(agree(r#"["ada", "bo"] | select(. == "ada")"#, None));
}

/// select can remove everything. Lua cannot tell an empty array from an empty record by
/// inspecting it, which is why the printer is built from the type instead.
#[test]
fn everything_filtered_out() {
    insta::assert_snapshot!(agree("[1, 2] | select(. >= 99)", None));
}

#[test]
fn the_target_program() {
    insta::assert_snapshot!(agree(ADULTS, Some(DB)));
}

#[test]
fn a_record_result() {
    let src = r#"
fn first(db: {u: {name: Str, age: Int}}) -> {name: Str, age: Int} = db.u

first(input)
"#;
    insta::assert_snapshot!(agree(src, Some(r#"{"u": {"name": "ada", "age": 36}}"#)));
}

/// Record keys come out in the type's order on every backend, not in the order the input
/// happened to list them. Asserted as an equality rather than as two snapshots that happen to
/// match, so that a backend reverting to insertion order fails here.
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

#[test]
fn strings_needing_escapes() {
    insta::assert_snapshot!(agree(r#"["say \"hi\"", "a\\b", "tab\there"]"#, None));
}

#[test]
fn emitted_js() {
    let p = toylang::compile(ADULTS).unwrap();
    insta::assert_snapshot!(toylang::emit_js::emit(&p));
}
