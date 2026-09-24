//! `map`, `sort_by` and `max_by` accept a closure value the way `select` does (a bare name
//! bound to a `$`-built function). What a misused one says is pinned here, since a program
//! that does not compile is not a corpus case.

fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

fn refuse(op: &str, elem: &str, f_ty: &str, ret: &str, arg: &str) -> String {
    err(&format!(
        "fn run({{ items, f }}: {{ items: Vec<{elem}>, f: {f_ty} }}) -> {ret} =\n  items | {op} f;\n\n\n\
         run {{ items: [], f: {arg} }}"
    ))
}

#[test]
fn map_refuses_a_closure_over_another_element_type() {
    insta::assert_snapshot!(refuse("map", "Int", "Str -> Int", "Vec<Int>", "length($)"));
}

#[test]
fn map_refuses_a_closure_whose_result_misses_the_expected_element() {
    insta::assert_snapshot!(refuse("map", "Int", "Int -> Int", "Vec<Str>", "$ + 1"));
}

#[test]
fn sort_by_refuses_a_closure_over_another_element_type() {
    insta::assert_snapshot!(refuse("sort_by", "Int", "Str -> Int", "Vec<Int>", "length($)"));
}

#[test]
fn sort_by_refuses_a_closure_that_returns_an_unordered_type() {
    insta::assert_snapshot!(refuse("sort_by", "Int", "Int -> Bool", "Vec<Int>", "$ > 1"));
}

#[test]
fn max_by_refuses_a_closure_over_another_element_type() {
    insta::assert_snapshot!(refuse("max_by", "Int", "Str -> Int", "Opt<Int>", "length($)"));
}

#[test]
fn max_by_refuses_a_closure_that_returns_an_unordered_type() {
    insta::assert_snapshot!(refuse("max_by", "Int", "Int -> Bool", "Opt<Int>", "$ > 1"));
}

/// A bare name that is not a function is not a closure: it keeps its `.`-rebinding reading,
/// so a Vec there is refused as a key, not as a "closure that is not callable".
#[test]
fn max_by_reads_a_non_function_name_as_a_key_expression() {
    insta::assert_snapshot!(refuse("max_by", "Int", "Vec<Int>", "Opt<Int>", "[1]"));
}

/// The same for `select`, whose closure spelling the others mirror: an Int name is a
/// predicate expression of the wrong type, and says so.
#[test]
fn select_reads_a_non_function_name_as_a_predicate_expression() {
    insta::assert_snapshot!(refuse("select", "Int", "Int", "Vec<Int>", "1"));
}
