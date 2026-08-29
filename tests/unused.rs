//! Unused bindings are compile errors, like Go (issue #45). Two binding forms: a function
//! parameter, and a match arm's destructured fields.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

#[test]
fn an_unused_parameter_is_refused() {
    insta::assert_snapshot!(err("fn f(x: Int) -> Int = 1\n\nf(1)"));
}

#[test]
fn a_read_parameter_compiles() {
    assert!(toylang::compile("fn f(x: Int) -> Int = x\n\nf(1)").is_ok());
}

/// The unused check is per-binding, not per-name: `x` shadows the outer name but is itself
/// read, so the inner function is fine even though an outer `x` bound the same word.
#[test]
fn a_parameter_used_only_in_a_nested_call_still_counts() {
    let src = "fn double(x: Int) -> Int = x + x\n\nfn f(x: Int) -> Int = double(x)\n\nf(1)";
    assert!(toylang::compile(src).is_ok());
}

#[test]
fn an_unused_destructured_field_is_refused() {
    insta::assert_snapshot!(err("enum Shape { point, circle{r: Int, color: Str} }\n\n\
         fn area(s: Shape) -> Int = s | circle{r, color} -> r * r or point -> 0\n\n\
         area(Shape.point)"));
}

#[test]
fn a_read_destructured_field_compiles() {
    let src = "enum Shape { point, circle{r: Int} }\n\n\
               fn area(s: Shape) -> Int = s | circle{r} -> r * r or point -> 0\n\n\
               area(Shape.point)";
    assert!(toylang::compile(src).is_ok());
}

/// A payload variant arm rebinds `.` to the payload itself, so `.r` and the bound name `r`
/// read the identical field off the identical local: there is no way to spell "read through
/// the subject, not the binding" that the checker could tell apart from using `r`.
#[test]
fn reading_the_field_through_the_subject_spelling_still_counts_as_using_the_binding() {
    let src = "enum Shape { circle{r: Int} }\n\n\
               fn area(s: Shape) -> Int = s | circle{r} -> .r * .r\n\n\
               area(circle{r: 3})";
    assert!(toylang::compile(src).is_ok());
}

/// `..` already closes the pattern, so the hint does not tell the reader to add what is
/// already there.
#[test]
fn the_hint_does_not_repeat_an_existing_rest() {
    insta::assert_snapshot!(err("enum Shape { circle{r: Int, color: Str} }\n\n\
         fn area(s: Shape) -> Int = s | circle{r, color, ..} -> r * r\n\n\
         area(circle{r: 3, color: \"red\"})"));
}
