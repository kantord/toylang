//! The refusal paths of the parameterized DSV source (gh:136). Behavior is corpus; what a
//! program cannot compile is this file's job, since nothing about it differs per backend.

fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// A source is legal only in the program's own body, `lines`/`inputs` included, so a function
/// spelling `dsv` is refused there the same way they are.
#[test]
fn dsv_in_a_function_body() {
    insta::assert_snapshot!(err("fn f() -> Vec<Vec<Str>> = dsv(\",\")\nf"));
}

/// There is only ever one real stdin, so a second `dsv` -- even with a different delimiter --
/// is refused rather than handed nothing.
#[test]
fn dsv_read_twice() {
    insta::assert_snapshot!(err("dsv(\",\") + dsv(\";\")"));
}

/// `dsv` splits the same raw lines `lines` reads, so the two cannot share one stdin.
#[test]
fn dsv_exclusive_with_lines() {
    insta::assert_snapshot!(err(
        "fn g(a: Stream<Str>) -> Vec<Str> = collect(a)\n{a: join_lines(g(lines)), b: csv}.a"
    ));
}

/// `dsv` reads the same stdin `input` reads whole, so the two cannot share one stdin.
#[test]
fn dsv_exclusive_with_input() {
    insta::assert_snapshot!(err(
        "fn g(a: {x: Str}) -> Str = a.x\n{g: g(input), c: csv}.g"
    ));
}

/// Every backend's split on an empty separator is its own undefined behaviour, so the empty
/// delimiter is refused up front rather than left to disagree at runtime.
#[test]
fn empty_delimiter_is_refused() {
    insta::assert_snapshot!(err("dsv(\"\")"));
}

/// A mapper body runs once per element, so a source read there would drain stdin on the first
/// element and hand every later one nothing.
#[test]
fn dsv_in_a_mapper_body() {
    insta::assert_snapshot!(err("dsv(\",\") | map(dsv(\";\"))"));
}
