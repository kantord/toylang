fn parse(src: &str) -> toylang::ast::File {
    toylang::parse::parse(src).unwrap()
}

/// The hoisted form (`fn name = body`, gh:152) parses with no parameter list and no return
/// annotation, and its body is a `MatchCall`. Step 2 parses only: the checker and codegen that
/// give the form meaning are the following steps, so this asserts on the tree.
#[test]
fn hoisted_call_form_declaration() {
    insta::assert_debug_snapshot!(parse(
        "enum Shape { point, circle{r: Int} }\nfn area = Shape(circle{r} -> r * r or point -> 0)\narea(Shape.point)"
    ));
}

/// A `MatchCall` is a type name used as a match over `.`, and it need not sit inside a hoisted
/// declaration: it is an expression in its own right.
#[test]
fn match_call_as_expression() {
    insta::assert_debug_snapshot!(parse(r#"Msg(Ping -> "pong" or Quit -> "bye")"#));
}

/// The parens hold the same `or`-separated arms a `Match` carries, so a payload-destructuring
/// arm and an `any()` default both parse inside the call.
#[test]
fn match_call_with_payload_and_default() {
    insta::assert_debug_snapshot!(parse(
        r#"Shape(circle{r} -> r * r or point -> 0 or any() -> 99)"#
    ));
}

/// A guard arm parses inside the call too, since the arms are the same ones a `Match` carries.
#[test]
fn match_call_with_guard() {
    insta::assert_debug_snapshot!(parse(r#"Msg(count > 3 -> "big" or count -> "small")"#));
}

/// The annotated `fn name(param: Type) -> Type = body` form is untouched by the hoisted branch.
#[test]
fn annotated_declaration_still_parses() {
    insta::assert_debug_snapshot!(parse(
        "enum Shape { point, circle{r: Int} }\nfn area(s: Shape) -> Int = s | circle{r} -> r * r or point -> 0\narea(Shape.point)"
    ));
}

/// A hoisted body is not required to be a match call at parse time: whether the checker
/// restricts it to one is the checker's step, not the parser's.
#[test]
fn hoisted_declaration_accepts_any_body() {
    insta::assert_debug_snapshot!(parse("fn answer = 42\nanswer"));
}
