//! The formatter's own correctness properties, checked against the corpus rather than a
//! hand-picked sample: idempotency (`fmt(fmt(x)) == fmt(x)`) and meaning-preservation (a
//! formatted program runs the same as the one it came from). Style itself -- what the output
//! actually looks like -- is pinned by the maintainer's own sample below, and enforced across
//! every example by `tests/fmt_examples.rs`.

mod support;

#[test]
fn every_corpus_program_formats_idempotently() {
    let cases = support::cases();
    assert!(
        !cases.is_empty(),
        "the corpus is empty, so this test proves nothing"
    );

    let mut failures = Vec::new();
    for case in &cases {
        let once = match toylang::fmt(&case.program) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{}: fmt failed: {e}", case.name));
                continue;
            }
        };
        let twice = match toylang::fmt(&once) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{}: fmt(fmt(x)) failed: {e}", case.name));
                continue;
            }
        };
        if once != twice {
            failures.push(format!(
                "{}: fmt is not idempotent\n--- fmt(x) ---\n{once}--- fmt(fmt(x)) ---\n{twice}",
                case.name
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} corpus programs failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
}

/// Formatting is a re-rendering, not a rewrite: it must never change what a program does.
/// Checked on Lua alone -- the corpus's cross-backend agreement is `corpus.rs`'s job, not this
/// one's, and re-running every backend here would only re-prove that agreement, not formatting.
#[test]
fn a_formatted_corpus_program_runs_the_same_as_the_original() {
    let cases = support::cases();
    let mut failures = Vec::new();

    for case in &cases {
        let formatted = match toylang::fmt(&case.program) {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{}: fmt failed: {e}", case.name));
                continue;
            }
        };
        let before = toylang::run_on(&case.program, case.input.as_deref(), toylang::Backend::Lua);
        let after = toylang::run_on(&formatted, case.input.as_deref(), toylang::Backend::Lua);
        match (before, after) {
            (Ok(a), Ok(b)) if a == b => {}
            (Err(_), Err(_)) => {}
            (before, after) => failures.push(format!(
                "{}: formatting changed behaviour: {before:?} -> {after:?}",
                case.name
            )),
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} corpus programs failed:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n\n")
    );
}

/// The maintainer's own sample (docs/examples/euler/01-multiples-of-3-and-5.md) is the one
/// ground truth for what the canonical style actually looks like -- everything else in
/// `emit_toylang.rs` is derived from or extends it. Pinned verbatim, not just checked for
/// idempotency, so a change to the layout rules cannot silently drift from it.
#[test]
fn the_maintainer_sample_formats_to_itself() {
    let sample = "fn triangle(m: Int) -> Int = m * (m + 1) / 2\n\
                  \n\
                  fn sum_of_multiples(p: {k: Int, limit: Int}) -> Int =\n\
                  \x20   triangle((p.limit - 1) / p.k) * p.k\n\
                  \n\
                  sum_of_multiples({k: 3, limit: 1000}) + sum_of_multiples({k: 5, limit: 1000}) -\n\
                  \x20   sum_of_multiples({k: 15, limit: 1000})\n";
    assert_eq!(toylang::fmt(sample).unwrap(), sample);
}

/// A pipeline that overflows the width breaks one stage per line, `|` leading each continuation
/// line so the pipes draw a vertical column (issue #101) -- the opposite of `Binary`'s trailing
/// rule, which pipelines used to follow by analogy before the maintainer pinned this shape.
#[test]
fn a_pipeline_that_does_not_fit_breaks_one_stage_per_line_pipe_first() {
    let src = "range(1000)\n\
               \x20   | select(. > 5)\n\
               \x20   | select(. < 1000 - somewhatlongvariablename)\n\
               \x20   | map(. * 2)\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
}

/// A Float literal has to come back out as a Float literal. The backends' shortest-digits
/// spelling drops the `.0` on a whole value and writes `1e21` as a digit run, and both of those
/// lex as `Int`, so the formatted program either changed type (`x * 2.0` became `x * 2`) or
/// stopped compiling (a 22-digit `Int` is out of range). Caught by the docs sweep the day the
/// Float reference page landed.
#[test]
fn a_float_literal_stays_a_float_literal() {
    let src = "fn f(x: Float) -> Float = x * 2.0\n\n[f(1.5), f(1e21), f(1e-7), f(0.25)]\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
    assert_eq!(toylang::fmt("1.0e21\n").unwrap(), "1e21\n");
    assert_eq!(toylang::fmt("3.0\n").unwrap(), "3.0\n");
}

/// Comments are the one input beside the tree (`emit_toylang`'s module doc has the placement
/// rules). Pinned directly since every corpus program is comment-free. Before comments were
/// recorded by the parser, only a leading banner survived, copied off the raw text, and every
/// comment between or inside declarations was dropped, which is why the Euler pages carry all
/// their explanation in a header.
#[test]
fn a_leading_comment_survives_formatting() {
    let src = "# Keep the elements that are at least 2.\n[1, 2, 3] | select(. >= 2)\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
}

/// A banner is told from a doc comment by the blank line after it, so that one blank line is
/// kept; a comment before a declaration, a `let` binding, or the program body goes above it,
/// and one trailing a line stays at the end of the line it lands on.
#[test]
fn a_commented_multi_function_program_formats_to_itself() {
    let src = "# A banner, separated from the first function by a blank line.\n\
               \n\
               # Doc comment on f.\n\
               fn f(x: Int) -> Int = x * 2 # trailing f\n\
               \n\
               fn g(x: Int) -> Int =\n\
               \x20   # before the binding\n\
               \x20   let a = f(x) # trailing the binding\n\
               \x20   # before the value\n\
               \x20   a + 1 # trailing the value\n\
               \n\
               # Before the program body.\n\
               g(1) # trailing the body\n\
               # After everything.\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
}

/// An expression is re-rendered from the tree, so a comment inside one has no line to stay on:
/// it rises to the top of the definition (or binding, or body) that holds it. Spacing and
/// parens around it are normalised like any other source.
#[test]
fn a_comment_inside_an_expression_rises_to_its_definition() {
    let src = "fn f(x: Int) -> Int =\n\
               \x20   x\n\
               \x20       # double every element\n\
               \x20       | map(. * 2) # then add them up\n\
               \x20       | sum\n\
               \n\
               f(1)\n";
    let want = "# double every element\n\
                # then add them up\n\
                fn f(x: Int) -> Int = x | map(. * 2) | sum\n\
                \n\
                f(1)\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// A module's comments go the same way, and a comment after the last declaration ends the file.
#[test]
fn a_commented_module_formats_to_itself() {
    let src = "# The module banner.\n\
               \n\
               pub fn f(x: Int) -> Int = x # trailing\n\
               \n\
               # Private helper.\n\
               fn g(x: Int) -> Int = f(x) + 1\n\
               # The end.\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
}

/// A module -- declarations with no trailing expression -- is a file shape the program parser
/// rejects outright, so a formatter that only knew programs could not format `prelude.toy`, the
/// one module in this repository and the first file a project-wide walk from the root reaches.
/// Pinned against the real file rather than a fixture: it is the file the feature exists for.
#[test]
fn the_prelude_is_a_module_and_is_already_formatted() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("prelude.toy");
    let src = std::fs::read_to_string(&path).expect("prelude.toy is readable");
    let formatted = toylang::fmt(&src).expect("prelude.toy formats");
    assert_eq!(
        formatted, src,
        "prelude.toy is not in canonical form -- run `toylang fmt --write`"
    );
    // A module has nothing to run: the formatter must not invent a body for one, and
    // `parse_module` is what proves the output is still a module rather than only looking like
    // one.
    toylang::parse::parse_module(&formatted).expect("the formatted prelude is still a module");
}
/// A lowercase head before `->` is a guard, decided by casing alone (`parse.rs`'s
/// `arm_starts_here`), so a Bool literal guard prints bare and reads back as the same guard.
/// Before the casing rule the paren-free `true -> ..` the formatter produced was a pattern
/// error, so `(true) -> ..` survived exactly one formatting pass.
#[test]
fn a_literal_guard_head_formats_bare_and_round_trips() {
    let src = "fn f(x: Int) -> Str = x | true -> \"a\" or \"b\"\n\nf(1)\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
    let parenthesized = "fn f(x: Int) -> Str = x | (true) -> \"a\" or \"b\"\n\nf(1)\n";
    assert_eq!(toylang::fmt(parenthesized).unwrap(), src);
    assert_eq!(
        toylang::run(parenthesized).unwrap(),
        toylang::run(src).unwrap()
    );
}

/// A signature that does not fit puts its parameter on its own line, and a record parameter
/// type that still does not fit there breaks one field per line; a broken signature never
/// takes its body on the closing line. Before this, the signature was the one node with no
/// seam, and Euler 11's `direction` sat at 128 columns.
#[test]
fn a_long_signature_breaks_at_its_parameter_then_inside_a_record_type() {
    let src = "fn four({g, r, c, dr, dc}: {g: Vec<Vec<Int>>, r: Int, c: Int, dr: Int, dc: Int}) -> Int = g[r]![c]!\n\nfour({g: [[1]], r: 0, c: 0, dr: 0, dc: 0})\n";
    let want = "fn four(\n\
                \x20   {g, r, c, dr, dc}: {g: Vec<Vec<Int>>, r: Int, c: Int, dr: Int, dc: Int}\n\
                ) -> Int =\n\
                \x20   g[r]![c]!\n\
                \n\
                four({g: [[1]], r: 0, c: 0, dr: 0, dc: 0})\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);

    let src = "fn direction({g, dr, dc, rmax, cmin, cmax}: {g: Vec<Vec<Int>>, dr: Int, dc: Int, rmax: Int, cmin: Int, cmax: Int}) -> Int = rmax\n\ndirection({g: [[1]], dr: 0, dc: 0, rmax: 0, cmin: 0, cmax: 0})\n";
    let want = "fn direction(\n\
                \x20   {g, dr, dc, rmax, cmin, cmax}: {\n\
                \x20       g: Vec<Vec<Int>>,\n\
                \x20       dr: Int,\n\
                \x20       dc: Int,\n\
                \x20       rmax: Int,\n\
                \x20       cmin: Int,\n\
                \x20       cmax: Int\n\
                \x20   }\n\
                ) -> Int =\n\
                \x20   rmax\n\
                \n\
                direction({g: [[1]], dr: 0, dc: 0, rmax: 0, cmin: 0, cmax: 0})\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// A match arm whose body does not fit breaks after its `->`, the body one level below the
/// column the chain's later arms sit at, so it is visibly deeper than the arm after it
/// (maintainer ruling, 2026-09-19); a bare default arm has no `->` and breaks in place. Arms
/// used to break only between each other, so an arm's own body overflowed.
#[test]
fn a_long_match_arm_breaks_after_its_arrow() {
    let src = "fn f(x: Int) -> Int = x | . > 1 -> some_function_call({alpha: x, beta: x + 1, gamma: x * 2, delta: x - 1, eps: x}) or 0\n\nfn some_function_call(p: {alpha: Int, beta: Int, gamma: Int, delta: Int, eps: Int}) -> Int = p.alpha\n\nf(1)\n";
    let want = "fn f(x: Int) -> Int =\n\
                \x20   x\n\
                \x20       | . > 1 ->\n\
                \x20                 some_function_call(\n\
                \x20                     {\n\
                \x20                         alpha: x,\n\
                \x20                         beta: x + 1,\n\
                \x20                         gamma: x * 2,\n\
                \x20                         delta: x - 1,\n\
                \x20                         eps: x\n\
                \x20                     }\n\
                \x20                 ) or\n\
                \x20             0\n\
                \n\
                fn some_function_call(\n\
                \x20   p: {alpha: Int, beta: Int, gamma: Int, delta: Int, eps: Int}\n\
                ) -> Int =\n\
                \x20   p.alpha\n\
                \n\
                f(1)\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// A postfix chain breaks inside its base, and the base's budget holds back the suffix's
/// columns: `[...][n]!` here is a list that fits at 80 exactly without `[n]!`, which used to
/// leave it compact and the line four columns over (Euler 17's `ones`).
#[test]
fn a_postfix_chain_breaks_inside_its_base_with_the_suffix_reserved() {
    let src = "fn ones(n: Int) -> Str =\n    [\"\", \"one\", \"two\", \"three\", \"four\", \"five\", \"six\", \"seven\", \"eight\", \"nine\"][n]!\n\nones(1)\n";
    let want = "fn ones(n: Int) -> Str =\n\
                \x20   [\n\
                \x20       \"\",\n\
                \x20       \"one\",\n\
                \x20       \"two\",\n\
                \x20       \"three\",\n\
                \x20       \"four\",\n\
                \x20       \"five\",\n\
                \x20       \"six\",\n\
                \x20       \"seven\",\n\
                \x20       \"eight\",\n\
                \x20       \"nine\"\n\
                \x20   ][n]!\n\
                \n\
                ones(1)\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// Comment text is normalised: one space after the `#` when the author wrote none, a bare `#`
/// left bare, and further indentation inside the text kept, since an indented line in a
/// comment is usually a list or a sample. Trailing whitespace never survives the parser.
#[test]
fn comment_text_gets_one_space_after_the_hash() {
    let src = "#no space\n#\n#   indented kept\nfn f(x: Int) -> Int = x #trail   \n\nf(1)\n";
    let want = "# no space\n#\n#   indented kept\nfn f(x: Int) -> Int = x # trail\n\nf(1)\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}
