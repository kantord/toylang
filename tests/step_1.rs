fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn hello_world() {
    insta::assert_snapshot!(toylang::run(r#""hello world""#).unwrap());
}

#[test]
fn emitted_lua() {
    let (lua, ty) = toylang::compile(r#""hello world""#).unwrap();
    insta::assert_snapshot!(format!("-- : {ty}\n{lua}"));
}

/// Escapes survive the trip through the Lua emitter rather than terminating the literal early.
#[test]
fn quotes_and_newlines_round_trip() {
    insta::assert_snapshot!(toylang::run(r#""say \"hi\"\n\tand \\ too""#).unwrap());
}

#[test]
fn unterminated_string() {
    insta::assert_snapshot!(err(r#""oops"#));
}

#[test]
fn trailing_garbage() {
    insta::assert_snapshot!(err(r#""a" "b""#));
}

#[test]
fn empty_program() {
    insta::assert_snapshot!(err("   "));
}
