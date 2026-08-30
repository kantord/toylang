fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn hello_world() {
    insta::assert_snapshot!(toylang::run(r#""hello world""#).unwrap());
}

#[test]
fn emitted_lua() {
    let p = toylang::compile(r#""hello world""#).unwrap();
    let (lua, ty) = (toylang::emit_lua::emit(&p), p.body.ty.clone());
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

/// Both edges at the dimension's own boundaries is the identity `[]` already is, so the
/// both-omitted slice is refused rather than carried as a spelling for it.
#[test]
fn slice_with_no_bounds_is_refused() {
    insta::assert_snapshot!(err("[1, 2, 3][:]"));
}
