const DB: &str = r#"{"users": [{"name": "ada", "age": 36}, {"name": "bo", "age": 9}]}"#;

const ADULTS: &str = r#"
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users | select(.age >= 18) | .[].name

adults(input)
"#;

fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

fn run_err(src: &str, stdin: &str) -> String {
    toylang::run_with_input(src, Some(stdin))
        .map(|_| ())
        .unwrap_err()
        .to_string()
}

#[test]
fn the_target_program() {
    insta::assert_snapshot!(toylang::run_with_input(ADULTS, Some(DB)).unwrap());
}

#[test]
fn emitted_lua() {
    let p = toylang::compile(ADULTS).unwrap();
    insta::assert_snapshot!(format!(
        "-- input : {}\n-- : {}\n{}",
        p.input.clone().unwrap(),
        p.body.ty,
        toylang::emit_lua::emit(&p)
    ));
}

/// A record's fields are a set, not a sequence (kantord/toylang#60): `g`'s parameter and `f`'s
/// parameter are the same type with the fields spelled in the other order, so the call
/// type-checks and the value reads back correctly on the other side.
#[test]
fn reordered_fields_are_the_same_type() {
    let src = r#"
fn f(r: {a: Str, b: Int}) -> Str = r.a
fn g(r: {b: Int, a: Str}) -> Str = f(r)

g(input)
"#;
    let out = toylang::run_with_input(src, Some(r#"{"b": 1, "a": "hi"}"#)).unwrap();
    assert_eq!(out, "hi\n");
}

#[test]
fn a_record_can_be_the_result() {
    let src = r#"
fn first(db: {u: {name: Str, age: Int}}) -> {name: Str, age: Int} = db.u

first(input)
"#;
    insta::assert_snapshot!(
        toylang::run_with_input(src, Some(r#"{"u": {"name": "ada", "age": 36}}"#)).unwrap()
    );
}

/// The payoff: a misspelled field is a compile error, which is the first thing here that jq
/// cannot structurally do.
#[test]
fn misspelled_field() {
    insta::assert_snapshot!(err(r#"
fn adults(db: {users: Vec<{name: Str, age: Int}>}) -> Vec<Str> =
    db.users | select(.age >= 18) | .[].nmae

adults(input)
"#));
}

#[test]
fn field_on_a_scalar() {
    insta::assert_snapshot!(err("fn f(x: Int) -> Int = x.name\nf(1)"));
}

/// `input` gets its type from the position it appears in, so with no position it has none.
#[test]
fn bare_input() {
    insta::assert_snapshot!(err("input"));
}

#[test]
fn input_used_at_two_types() {
    insta::assert_snapshot!(err(r#"
fn a(x: Int) -> Str = x | "n"
fn b(x: Str) -> Str = x

a(input) + b(input)
"#));
}

#[test]
fn duplicate_record_field() {
    insta::assert_snapshot!(err("fn f(r: {a: Str, a: Int}) -> Str = r.a\nf(input)"));
}

#[test]
fn input_is_not_coerced() {
    insta::assert_snapshot!(run_err(
        ADULTS,
        r#"{"users": [{"name": "ada", "age": "36"}]}"#
    ));
}

#[test]
fn input_missing_a_field() {
    insta::assert_snapshot!(run_err(ADULTS, r#"{"users": [{"name": "ada"}]}"#));
}

#[test]
fn input_is_the_wrong_shape() {
    insta::assert_snapshot!(run_err(ADULTS, r#"{"users": {"name": "ada"}}"#));
}

/// A float where Int was declared is an error, not a truncation.
#[test]
fn input_float_where_int_declared() {
    insta::assert_snapshot!(run_err(
        ADULTS,
        r#"{"users": [{"name": "ada", "age": 36.5}]}"#
    ));
}

/// Fields the program did not declare are ignored, so a program can read two fields off a log
/// line without describing the whole line.
#[test]
fn undeclared_input_fields_are_ignored() {
    let stdin = r#"{"users": [{"name": "ada", "age": 36, "email": "a@b.c"}], "version": 3}"#;
    insta::assert_snapshot!(toylang::run_with_input(ADULTS, Some(stdin)).unwrap());
}

#[test]
fn program_needs_input_but_none_given() {
    insta::assert_snapshot!(toylang::run(ADULTS).map(|_| ()).unwrap_err().to_string());
}

/// A non-`pub` prelude helper is a private helper for prelude.toy: it stays available to the
/// `pub` functions in that file, but the program's own file is a different file, so a call to it
/// from here is refused (gh:166).
#[test]
fn a_program_cannot_call_a_private_prelude_helper() {
    insta::assert_snapshot!(err(r#"join_parts(["ada", "bo"])"#));
}

/// The same-file side of that rule: a program's own non-`pub` function is in the same file as
/// its call site, so calling it is legal. The refusal only ever crosses a file boundary.
#[test]
fn a_program_can_call_its_own_non_pub_helper() {
    let src = r#"
fn parts(v: Vec<Str>) -> Str =
    v | length(v) == 0 -> "" or v[0]! + parts(tail(v)!)

parts(["ada", "bo"])
"#;
    insta::assert_snapshot!(toylang::run(src).unwrap());
}
