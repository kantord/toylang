fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn call_a_function() {
    let src = r#"
fn greet(who: Str) -> Str = "hello " + who

greet("world")
"#;
    insta::assert_snapshot!(toylang::run(src).unwrap());
}

/// Signatures are collected before any body is checked, so definition order does not matter.
#[test]
fn call_a_function_defined_later() {
    let src = r#"
fn outer(x: Str) -> Str = inner(x) + "!"
fn inner(x: Str) -> Str = "[" + x + "]"

outer("hi")
"#;
    insta::assert_snapshot!(toylang::run(src).unwrap());
}

/// A toylang function may be called `print`. The emitter prefixes every name so the one in the
/// generated Lua does not shadow the host's.
#[test]
fn a_function_may_be_named_print() {
    let src = r#"
fn print(x: Str) -> Str = "got " + x

print("it")
"#;
    insta::assert_snapshot!(toylang::run(src).unwrap());
}

#[test]
fn emitted_lua() {
    let src = r#"
fn greet(who: Str) -> Str = "hello " + who

greet("world")
"#;
    let p = toylang::compile(src).unwrap();
    let (lua, ty) = (toylang::emit_lua::emit(&p), p.body.ty.clone());
    insta::assert_snapshot!(format!("-- : {ty}\n{lua}"));
}

#[test]
fn argument_type_mismatch() {
    insta::assert_snapshot!(err(r#"fn greet(who: Str) -> Str = who
greet(42)"#));
}

#[test]
fn return_type_mismatch() {
    insta::assert_snapshot!(err(r#"fn f(x: Str) -> Int = x
f("a")"#));
}

/// The check step 2 deferred: `+` is Str concatenation, and Int now exists to violate it.
#[test]
fn concat_an_int() {
    insta::assert_snapshot!(err(r#""a" + 1"#));
}

#[test]
fn parameter_without_annotation() {
    insta::assert_snapshot!(err(r#"fn greet(who) -> Str = who
greet("x")"#));
}

#[test]
fn function_without_return_type() {
    insta::assert_snapshot!(err(r#"fn greet(who: Str) = who
greet("x")"#));
}

#[test]
fn unknown_type_name() {
    insta::assert_snapshot!(err(r#"fn greet(who: Text) -> Text = who
greet("x")"#));
}

#[test]
fn unbound_name() {
    insta::assert_snapshot!(err(r#"fn greet(who: Str) -> Str = other
greet("x")"#));
}

#[test]
fn call_of_an_undefined_function() {
    insta::assert_snapshot!(err(r#"nope("x")"#));
}

#[test]
fn a_parameter_is_not_a_function() {
    insta::assert_snapshot!(err(r#"fn greet(who: Str) -> Str = who("x")
greet("x")"#));
}

#[test]
fn duplicate_definition() {
    insta::assert_snapshot!(err(r#"fn f(x: Str) -> Str = x
fn f(x: Str) -> Str = x
f("a")"#));
}

/// A parameter is in scope in its own body and nowhere else.
#[test]
fn parameter_does_not_leak() {
    insta::assert_snapshot!(err(r#"fn f(x: Str) -> Str = x
x"#));
}

/// `fn name() -> T = body`, called `name()` (issue #61): a function may take no parameter.
#[test]
fn a_nullary_function() {
    let src = r#"
fn greeting() -> Str = "hello"

greeting()
"#;
    insta::assert_snapshot!(toylang::run(src).unwrap());
}

#[test]
fn a_nullary_function_called_with_an_argument() {
    insta::assert_snapshot!(err(r#"fn greeting() -> Str = "hello"
greeting("x")"#));
}

#[test]
fn a_unary_function_called_with_no_argument() {
    insta::assert_snapshot!(err(r#"fn greet(who: Str) -> Str = who
greet()"#));
}

/// A hoisted definition (`fn name = expr`, gh:152) may write any body, but the checker insists
/// that body be a match call naming the enum the implicit `.` parameter matches; a bare
/// expression gives it nothing to infer a parameter type from.
#[test]
fn hoisted_def_needs_a_match_call_body() {
    insta::assert_snapshot!(err(r#"fn nope = 42

nope(1)"#));
}

/// The match call in a hoisted body names an enum that must exist: an unknown name is an
/// unknown type, not a plausible-but-wrong signature.
#[test]
fn hoisted_def_unknown_enum() {
    insta::assert_snapshot!(err(r#"fn nope = Missing(any() -> 0)

nope(1)"#));
}

/// The stream invariant holds for the inferred return too: a hoisted body cannot conjure
/// a stream without taking one in through a parameter, same as a written signature (gh:152).
#[test]
fn hoisted_def_cannot_conjure_a_stream() {
    insta::assert_snapshot!(err(
        r#"enum Msg { Ping }
fn render = Msg(Ping -> range(3))

collect(render(Msg.ping))"#
    ));
}
