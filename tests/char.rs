//! `Char` in the type grammar (kantord/toylang#75): a single Unicode scalar value, reachable
//! only through `chars`. This file is about the annotation and the two wire-form refusals; the
//! behavioral cases (what `chars` decodes to, how ranges and complement are written) live in
//! tests/corpus.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn a_function_can_take_and_return_a_char() {
    assert!(toylang::compile("fn id(c: Char) -> Char = c\n\nlength(chars(\"a\"))").is_ok());
}

#[test]
fn a_record_field_can_be_a_char() {
    assert!(toylang::compile("fn f(r: {c: Char}) -> Char = r.c\n\n1").is_ok());
}

#[test]
fn a_vec_can_hold_a_char() {
    assert!(toylang::compile("fn f(v: Vec<Char>) -> Int = length(v)\n\n1").is_ok());
}

#[test]
fn input_cannot_be_char_typed() {
    insta::assert_snapshot!(err("fn f(c: Char) -> Bool = c == c\n\nf(input)"));
}

#[test]
fn inputs_cannot_carry_a_char_element() {
    insta::assert_snapshot!(err(
        "fn f(v: Vec<Char>) -> Int = length(v)\n\nf(collect(inputs))"
    ));
}

#[test]
fn the_programs_result_cannot_be_a_char() {
    insta::assert_snapshot!(err("chars(\"a\")[0]!"));
}

#[test]
fn jsonlines_cannot_print_a_char() {
    insta::assert_snapshot!(err("jsonlines(chars(\"a\"))"));
}
