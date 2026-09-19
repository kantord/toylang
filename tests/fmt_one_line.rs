//! The one-line template (`toylang::fmt_one_line`): the same tree `fmt` renders, on a single
//! line. Its correctness property is that both templates render one tree, so the file form of
//! the one-line form is the file form of the original; its known limit is the grammar's, not
//! the template's (see `fmt_one_line`'s doc).

mod support;

/// The file form of `src` with its comments dropped: what the one-line form, which carries
/// none, has to round-trip to.
fn file_form_without_comments(src: &str) -> String {
    let mut file = toylang::parse::parse(src).expect("a program");
    file.comments.clear();
    toylang::fmt::emit(&file)
}

enum Outcome {
    Rendered,
    Refused,
    Failed(String),
}

/// One corpus program through the one-line template: rendered and checked (one line, the
/// same tree as the file form, idempotent, runs the same on Lua), refused with the grammar's
/// reason, or failed with what went wrong.
fn one_line_outcome(case: &support::Case) -> Outcome {
    let line = match toylang::fmt_one_line(&case.program) {
        Ok(line) => line,
        Err(e) if e.msg.contains("no one-line form") => return Outcome::Refused,
        Err(e) => return Outcome::Failed(e.to_string()),
    };
    if line.matches('\n').count() != 1 || !line.ends_with('\n') {
        return Outcome::Failed(format!("not one line:\n{line}"));
    }
    if toylang::fmt(&line).unwrap() != file_form_without_comments(&case.program) {
        return Outcome::Failed("the one-line form is a different tree".to_string());
    }
    if toylang::fmt_one_line(&line).unwrap() != line {
        return Outcome::Failed("not idempotent".to_string());
    }
    let before = toylang::run_on(&case.program, case.input.as_deref(), toylang::Backend::Lua);
    let after = toylang::run_on(&line, case.input.as_deref(), toylang::Backend::Lua);
    match (before, after) {
        (Ok(a), Ok(b)) if a == b => Outcome::Rendered,
        (Err(_), Err(_)) => Outcome::Rendered,
        (before, after) => Outcome::Failed(format!(
            "the one-line form runs differently: {before:?} -> {after:?}"
        )),
    }
}

#[test]
fn every_corpus_program_has_a_one_line_form_or_says_why_not() {
    let cases = support::cases();
    let mut rendered = 0;
    let mut refused = Vec::new();
    let mut failures = Vec::new();
    for case in &cases {
        match one_line_outcome(case) {
            Outcome::Rendered => rendered += 1,
            Outcome::Refused => refused.push(case.name.clone()),
            Outcome::Failed(why) => failures.push(format!("{}: {why}", case.name)),
        }
    }
    assert!(rendered > 0, "no corpus program rendered on one line");
    assert!(
        failures.is_empty(),
        "{} of {} failed ({} refused: {}):\n{}",
        failures.len(),
        cases.len(),
        refused.len(),
        refused.join(", "),
        failures.join("\n\n")
    );
}

/// Every kind of declaration on one line; the body of the last definition ends in a call, so
/// the program body after it cannot be read as its argument. Comments are dropped: none can
/// sit inside a line. A rendering check only; that a one-line form runs the same is the corpus
/// test's claim.
#[test]
fn every_kind_of_declaration_renders_on_one_line() {
    let src = "# dropped\n\
               type P = {a: Int, b: Int}\n\
               \n\
               enum Shape { Circle(Int), Point }\n\
               \n\
               trait Area {\n\
               \x20   fn area(s: Shape) -> Int\n\
               }\n\
               \n\
               impl Area for Shape {\n\
               \x20   fn area(s: Shape) -> Int = s | Circle(r) -> r * r * 3 or 0\n\
               }\n\
               \n\
               # dropped too\n\
               fn f(p: P) -> Int = p.a * 2 + p.b\n\
               \n\
               fn g(x: Int) -> Int = f({a: x, b: x})\n\
               \n\
               g(1)\n";
    let want = "type P = {a: Int, b: Int} enum Shape { Circle(Int), Point } \
                trait Area { fn area(s: Shape) -> Int } \
                impl Area for Shape { fn area(s: Shape) -> Int = s | Circle(r) -> r * r * 3 or 0 } \
                fn f(p: P) -> Int = p.a * 2 + p.b \
                fn g(x: Int) -> Int = f({a: x, b: x}) g(1)\n";
    assert_eq!(toylang::fmt_one_line(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), file_form_without_comments(src));
}

/// Two shapes have no one-line form by ruling (2026-09-19): a body ending in a name followed
/// on the same line by the program body, which reads as a bare application; and any `let`
/// block, which the grammar reads one binding per line. Both are refused with the definition
/// named.
#[test]
fn a_body_that_would_swallow_the_program_body_is_refused() {
    let src = "fn g(x: Int) -> Int = x\n\ng(1)\n";
    let err = toylang::fmt_one_line(src).unwrap_err();
    assert!(err.msg.contains("no one-line form"), "{}", err.msg);
    assert_eq!(&src[err.span.start..err.span.end], "fn g(x: Int) -> Int = x");

    // A `let` block is refused outright, even one that would happen to read back the same.
    let src = "fn f(x: Int) -> Int =\n    let a = x * 2\n    a + 1\n\nf(1)\n";
    let err = toylang::fmt_one_line(src).unwrap_err();
    assert!(err.msg.contains("`let` block"), "{}", err.msg);
    assert_eq!(err.span.start, 0);
}

/// A module is declarations only, each opening with a keyword, so it always has a one-line
/// form; pinned on the real prelude.
#[test]
fn the_prelude_renders_on_one_line_and_round_trips() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prelude.toy");
    let src = std::fs::read_to_string(&path).expect("prelude.toy is readable");
    let line = toylang::fmt_one_line(&src).expect("prelude.toy renders on one line");
    assert_eq!(line.matches('\n').count(), 1);
    assert_eq!(toylang::fmt(&line).unwrap(), src);
}
