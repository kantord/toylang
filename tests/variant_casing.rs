//! The gh:156 casing split: the declared variant name is the matcher and starts with a capital
//! letter; the value is built with the lowercase constructor. The corpus carries the accept
//! case across every backend; these pin what the corpus cannot see, the checker's refusals.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// A lowercase declaration names a value, but a variant declaration is a matcher, so it must
/// start with a capital letter.
#[test]
fn a_lowercase_variant_declaration_is_refused() {
    insta::assert_snapshot!(err("enum Shape { point, circle{r: Int} }\n\nstr(1)"));
}

/// A capitalized name builds nothing: it is the matcher, so using it in value position is
/// refused, and the lowercase constructor is the way to build.
#[test]
fn a_capitalized_constructor_is_refused() {
    insta::assert_snapshot!(err(
        "enum Shape { Point, Circle{r: Int} }\n\nCircle{r: 1}"
    ));
}

/// A lowercase pattern names the constructor, not a matcher, so it cannot be matched against.
#[test]
fn a_lowercase_pattern_is_refused() {
    insta::assert_snapshot!(err(
        "enum Shape { Point, Circle{r: Int} }\n\n\
         fn f(s: Shape) -> Int = s | circle{r} -> r or Point -> 0\n\n\
         f(Shape.point)"
    ));
}

/// The accept case end to end: a capitalized declaration with a lowercase constructor builds
/// the value, and the capitalized matcher matches it.
#[test]
fn capitalized_declaration_with_lowercase_constructor_runs() {
    let src = "enum Shape { Point, Circle{r: Int} }\n\nfn area(s: Shape) -> Int = \
               s | Circle{r} -> r * r or Point -> 0\n\n{a: area(Shape.point), b: area(circle{r: 3})}";
    assert_eq!(toylang::run(src).unwrap(), "{\"a\":0,\"b\":9}\n");
}
/// Casing decides an arm head before any name resolves (`parse.rs`'s `arm_starts_here`): a
/// capitalized head is a pattern even when nothing declares it, so it fails as a pattern --
/// no enum subject, or no such variant -- rather than being retried as an expression.
#[test]
fn a_capitalized_head_over_a_non_enum_subject_is_a_pattern_error() {
    insta::assert_snapshot!(err("fn f(x: Int) -> Int = x | Yes -> 1 or 2\n\nf(1)"));
}

#[test]
fn a_capitalized_head_naming_no_variant_is_a_pattern_error() {
    insta::assert_snapshot!(err("enum Shape { Point, Circle{r: Int} }\n\n\
         fn f(s: Shape) -> Int = s | Square -> 1 or 2\n\n\
         f(Shape.point)"));
}

/// A lowercase head is a guard, so a bare unit constructor written where its matcher was meant
/// reaches the checker as an expression; the hint is the same one `circle{r}` gets.
#[test]
fn a_lowercase_unit_constructor_head_gets_the_matcher_hint() {
    insta::assert_snapshot!(err("enum Shape { Point, Circle{r: Int} }\n\n\
         fn f(s: Shape) -> Int = s | point -> 0 or Circle{r} -> r\n\n\
         f(Shape.point)"));
}
