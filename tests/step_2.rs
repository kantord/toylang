fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn concat() {
    insta::assert_snapshot!(toylang::run(r#""hello " + "world""#).unwrap());
}

#[test]
fn concat_chain() {
    insta::assert_snapshot!(toylang::run(r#""a" + "b" + "c" + "d""#).unwrap());
}

/// Associativity is invisible in the output, because `+` on strings is associative and the
/// emitter flattens. The parse tree is the only place it shows, and it is what the precedence
/// table is for, so assert on the tree: this must nest left, not right.
#[test]
fn concat_is_left_associative() {
    let tokens = toylang::lex::lex(r#""a" + "b" + "c""#).unwrap();
    insta::assert_debug_snapshot!(toylang::parse::parse(&tokens).unwrap());
}

#[test]
fn emitted_lua() {
    let lua = toylang::compile(r#""a" + "b" + "c""#).unwrap().lua;
    insta::assert_snapshot!(lua);
}

#[test]
fn missing_right_operand() {
    insta::assert_snapshot!(err(r#""a" + "#));
}

#[test]
fn leading_operator() {
    insta::assert_snapshot!(err(r#"+ "a""#));
}
