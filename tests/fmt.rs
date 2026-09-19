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

/// The maintainer's own hand-formatted sample (examples/shapes.toy, 2026-09-19) is the one
/// ground truth for what the canonical style looks like -- 2-space indent, padded braces, a
/// chain on one line when it fits -- and everything else in `src/fmt/multi_line.rs` is derived
/// from or extends it. Pinned verbatim, not just checked for idempotency, so a change to the
/// layout rules cannot silently drift from it. The earlier sample (Euler 1) set the rules this
/// one replaced: 4-space indent, 80 columns, unpadded records.
#[test]
fn the_maintainer_sample_formats_to_itself() {
    let sample = "# An enum with a payload variant, consumed by an exhaustive match.\n\
                  enum Shape { Point, Circle { r: Int } }\n\
                  \n\n\
                  fn area_ish(s: Shape) -> Int =\n\
                  \x20 s | Circle { r } -> r * r or Point -> 0\n\
                  \n\n\
                  { a: area_ish Shape.point, b: area_ish(circle { r: 3 }) }\n";
    assert_eq!(toylang::fmt(sample).unwrap(), sample);
    let on_disk = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/shapes.toy"),
    )
    .expect("examples/shapes.toy is readable");
    assert_eq!(on_disk, sample, "examples/shapes.toy is the pinned sample");
}

/// A pipeline that overflows the width breaks one stage per line, `|` leading each continuation
/// line at the subject's own column so the pipes draw a vertical column (issue #101, then the
/// 2026-09-19 sample for the column) -- the opposite of `Binary`'s trailing rule.
#[test]
fn a_pipeline_that_does_not_fit_breaks_one_stage_per_line_pipe_first() {
    let src = "range 1000\n\
               | select(. > 5)\n\
               | select(. < 1000 - somewhatlongvariablename)\n\
               | map(. * 2)\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
}

/// A Float literal has to come back out as a Float literal. The backends' shortest-digits
/// spelling drops the `.0` on a whole value and writes `1e21` as a digit run, and both of those
/// lex as `Int`, so the formatted program either changed type (`x * 2.0` became `x * 2`) or
/// stopped compiling (a 22-digit `Int` is out of range). Caught by the docs sweep the day the
/// Float reference page landed.
#[test]
fn a_float_literal_stays_a_float_literal() {
    let src = "fn f(x: Float) -> Float = x * 2.0\n\n\n[f 1.5, f 1e21, f 1e-7, f 0.25]\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
    assert_eq!(toylang::fmt("1.0e21\n").unwrap(), "1e21\n");
    assert_eq!(toylang::fmt("3.0\n").unwrap(), "3.0\n");
}

/// Comments are the one input beside the tree (`src/fmt/comments.rs`'s module doc has the placement
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
               \n\n\
               fn g(x: Int) -> Int =\n\
               \x20 # before the binding\n\
               \x20 let a = f x # trailing the binding\n\
               \n\
               \x20 # before the value\n\
               \x20 a + 1 # trailing the value\n\
               \n\n\
               # Before the program body.\n\
               g 1 # trailing the body\n\
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
               \n\n\
               f(1)\n";
    let want = "# double every element\n\
                # then add them up\n\
                fn f(x: Int) -> Int = x | map(. * 2) | sum\n\
                \n\n\
                f 1\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// A module's comments go the same way, and a comment after the last declaration ends the file.
#[test]
fn a_commented_module_formats_to_itself() {
    let src = "# The module banner.\n\
               \n\
               pub fn f(x: Int) -> Int = x # trailing\n\
               \n\n\
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
    let src = "fn f(x: Int) -> Str = x | true -> \"a\" or \"b\"\n\n\nf 1\n";
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
/// seam, and Euler 11's `direction` sat at 128 columns. (`four`'s parameter fit on its own
/// line at 80 columns; at 69 it breaks too.)
#[test]
fn a_long_signature_breaks_at_its_parameter_then_inside_a_record_type() {
    let src = "fn four({g, r, c, dr, dc}: {g: Vec<Vec<Int>>, r: Int, c: Int, dr: Int, dc: Int}) -> Int = g[r]![c]!\n\nfour({g: [[1]], r: 0, c: 0, dr: 0, dc: 0})\n";
    let want = "fn four(\n\
                \x20 { g, r, c, dr, dc }: {\n\
                \x20   g: Vec<Vec<Int>>,\n\
                \x20   r: Int,\n\
                \x20   c: Int,\n\
                \x20   dr: Int,\n\
                \x20   dc: Int\n\
                \x20 }\n\
                ) -> Int =\n\
                \x20 g[r]![c]!\n\
                \n\n\
                four { g: [[1]], r: 0, c: 0, dr: 0, dc: 0 }\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);

    let src = "fn direction({g, dr, dc, rmax, cmin, cmax}: {g: Vec<Vec<Int>>, dr: Int, dc: Int, rmax: Int, cmin: Int, cmax: Int}) -> Int = rmax\n\ndirection({g: [[1]], dr: 0, dc: 0, rmax: 0, cmin: 0, cmax: 0})\n";
    let want = "fn direction(\n\
                \x20 { g, dr, dc, rmax, cmin, cmax }: {\n\
                \x20   g: Vec<Vec<Int>>,\n\
                \x20   dr: Int,\n\
                \x20   dc: Int,\n\
                \x20   rmax: Int,\n\
                \x20   cmin: Int,\n\
                \x20   cmax: Int\n\
                \x20 }\n\
                ) -> Int =\n\
                \x20 rmax\n\
                \n\n\
                direction { g: [[1]], dr: 0, dc: 0, rmax: 0, cmin: 0, cmax: 0 }\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// A match arm whose body does not fit breaks after its `->`, the body one level in from the
/// arms, which all share the first arm's column (maintainer ruling, 2026-09-19); a bare default
/// arm has no `->` and breaks in place. Arms used to break only between each other, so an
/// arm's own body overflowed.
#[test]
fn a_long_match_arm_breaks_after_its_arrow() {
    let src = "fn f(x: Int) -> Int = x | . > 1 -> some_function_call({alpha: x, beta: x + 1, gamma: x * 2, delta: x - 1, eps: x}) or 0\n\nfn some_function_call(p: {alpha: Int, beta: Int, gamma: Int, delta: Int, eps: Int}) -> Int = p.alpha\n\nf(1)\n";
    let want = "fn f(x: Int) -> Int =\n\
                \x20 x\n\
                \x20 | . > 1 ->\n\
                \x20     some_function_call {\n\
                \x20       alpha: x,\n\
                \x20       beta: x + 1,\n\
                \x20       gamma: x * 2,\n\
                \x20       delta: x - 1,\n\
                \x20       eps: x\n\
                \x20     } or\n\
                \x20   0\n\
                \n\n\
                fn some_function_call(\n\
                \x20 p: { alpha: Int, beta: Int, gamma: Int, delta: Int, eps: Int }\n\
                ) -> Int =\n\
                \x20 p.alpha\n\
                \n\n\
                f 1\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// A postfix chain breaks inside its base, and the base's budget holds back the suffix's
/// columns: `[...][n]!` here is a list that once fit the width exactly without `[n]!`, which
/// used to leave it compact and the line four columns over (Euler 17's `ones`).
#[test]
fn a_postfix_chain_breaks_inside_its_base_with_the_suffix_reserved() {
    let src = "fn ones(n: Int) -> Str =\n    [\"\", \"one\", \"two\", \"three\", \"four\", \"five\", \"six\", \"seven\", \"eight\", \"nine\"][n]!\n\nones(1)\n";
    let want = "fn ones(n: Int) -> Str =\n\
                \x20 [\n\
                \x20   \"\",\n\
                \x20   \"one\",\n\
                \x20   \"two\",\n\
                \x20   \"three\",\n\
                \x20   \"four\",\n\
                \x20   \"five\",\n\
                \x20   \"six\",\n\
                \x20   \"seven\",\n\
                \x20   \"eight\",\n\
                \x20   \"nine\"\n\
                \x20 ][n]!\n\
                \n\n\
                ones 1\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// Comment text is normalised: one space after the `#` when the author wrote none, a bare `#`
/// left bare, and further indentation inside the text kept, since an indented line in a
/// comment is usually a list or a sample. Trailing whitespace never survives the parser.
#[test]
fn comment_text_gets_one_space_after_the_hash() {
    let src = "#no space\n#\n#   indented kept\nfn f(x: Int) -> Int = x #trail   \n\nf(1)\n";
    let want = "# no space\n#\n#   indented kept\nfn f(x: Int) -> Int = x # trail\n\n\nf 1\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// Two negations print with a space between them: `--5` re-parses the same, but reads as a
/// decrement.
#[test]
fn a_nested_negation_keeps_a_space_between_the_signs() {
    assert_eq!(toylang::fmt("-(-5)\n").unwrap(), "- -5\n");
    assert_eq!(toylang::fmt("- -5\n").unwrap(), "- -5\n");
    assert_eq!(
        toylang::run("- -5\n").unwrap(),
        toylang::run("-(-5)\n").unwrap()
    );
}

/// A record field whose value does not fit beside its name drops the value to its own line
/// one level in, unless the value opens a bracket, which stays on the name's line. Before
/// this the value's budget ignored the `name: ` prefix, so Euler 19's `weekday` field sat
/// four columns over the width.
#[test]
fn a_record_field_whose_value_does_not_fit_breaks_after_the_name() {
    let src = "{ month: month == 12 | . -> 1 or month + 1, weekday: (weekday + days_in_month({ month: month, year: year })) % 7, rows: { a: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20] } }\n";
    let want = "{\n\
                \x20 month: month == 12 | . -> 1 or month + 1,\n\
                \x20 weekday:\n\
                \x20   (weekday + days_in_month { month: month, year: year }) % 7,\n\
                \x20 rows: {\n\
                \x20   a: [\n\
                \x20     1,\n\
                \x20     2,\n\
                \x20     3,\n\
                \x20     4,\n\
                \x20     5,\n\
                \x20     6,\n\
                \x20     7,\n\
                \x20     8,\n\
                \x20     9,\n\
                \x20     10,\n\
                \x20     11,\n\
                \x20     12,\n\
                \x20     13,\n\
                \x20     14,\n\
                \x20     15,\n\
                \x20     16,\n\
                \x20     17,\n\
                \x20     18,\n\
                \x20     19,\n\
                \x20     20\n\
                \x20   ]\n\
                \x20 }\n\
                }\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
}

/// A call around a pipeline stays a call. `join_lines(collect(P))` reads better as
/// `P | collect(.) | join_lines(.)`, and the two run the same when both check, but they are
/// not one tree to the checker (found 2026-09-19 by trying the rewrite on the corpus, where it
/// broke ten programs): a sink such as `jsonlines` is legal only as the outermost expression,
/// never as a pipe stage, and `f(P)` pushes `f`'s parameter type down into `P`, which is what
/// lets a `parse(.)` inside it check, while a pipe stage gets no expected type, even inside
/// a call's argument. The formatter has no types, so it must not rewrite one into the other.
#[test]
fn a_call_around_a_pipeline_keeps_its_call_form() {
    let src =
        "fn total(nums: Vec<Int>) -> Int = length nums\n\n\ntotal collect(stdin | map parse(.))\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
    assert!(toylang::run_with_input(src, Some("1\n2\n")).is_ok());
    let as_stages = "fn total(nums: Vec<Int>) -> Int = length(nums)\n\nstdin | map(parse(.)) | collect(.) | total(.)\n";
    assert!(toylang::run_with_input(as_stages, Some("1\n2\n")).is_err());
    let sink = "range(3) | map(. * 10) | jsonlines(.)\n";
    assert!(toylang::run(sink).is_err());
}

/// Bare application, `f x`, is the default call form (maintainer ruling, 2026-09-19); `f(x)`
/// stays only where the grammar cannot read `f x` back as one tree -- `src/fmt/one_line.rs`'s
/// `bare_arg_ok` mirrors `parse.rs::ident_expr` exactly, not "looks simple enough".
#[test]
fn a_call_argument_prints_bare_wherever_the_grammar_reads_it_back() {
    // Str, Int, Float, a name, a record literal, and a nested call are all safe bare, and
    // chain right-associatively with no first-class functions to make the reading ambiguous.
    let src = "fn f(x: Int) -> Int = x * 10\n\n\nfn g(x: Int) -> Int = x + 1\n\n\n\
               [f 1, f \"s\", f 1.5, f x, f { a: 1 }, f(g 2)]\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);

    // A postfix chain on top of a safe base reads back as part of the SAME argument (the
    // grammar's bare-argument path is `self.postfix()`, the same trailer loop `.field`/`[i]`/
    // `!`/`:method(...)` use everywhere else), so all three stay bare and run the same.
    let src = "fn f(x: Int) -> Int = x\n\
               \n\n\
               fn r(x: Int) -> { a: Int } = { a: x }\n\
               \n\n\
               trait M {\n\
               \x20 fn m(p: { x: Self, y: Int }) -> Self\n\
               }\n\
               impl M for Int {\n\
               \x20 fn m(p: { x: Self, y: Int }) -> Self = p.x + p.y\n\
               }\n\
               \n\n\
               [f(r(1).a), f([1, 2][0]!), f(1:m({ x: 1, y: 2 }))]\n";
    let want = src
        .replace(
            "[f(r(1).a), f([1, 2][0]!), f(1:m({ x: 1, y: 2 }))]",
            "[f r(1).a, f([1, 2][0]!), f 1:m({ x: 1, y: 2 })]",
        )
        .replace("}\nimpl M for Int", "}\n\n\nimpl M for Int");
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(&want).unwrap(), want);
    assert_eq!(toylang::run(src).unwrap(), toylang::run(&want).unwrap());

    // The grammar's own exclusions: `-` is always subtraction, `[` always indexes (so a Vec
    // literal argument needs parens with or without a postfix chain on top: `parse.rs`'s
    // `self.argument()` for a *record* literal is the one exception that continues no further,
    // but `[` is never even a recognised argument start), and `.` is always field access on
    // the callee.
    let src = "fn f(x: Int) -> Int = x\n\n\n[f(-1), f([1, 2]), f([1, 2][0]!), f(.)]\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);

    // A record literal *with* a postfix chain on it is the one base that does not continue:
    // `self.argument()`'s LBrace case never reads a trailer, so `{a: 1}.a` written bare would
    // reattach the `.a` to the call's result. Alone, with no chain, it is bare-safe (tested
    // above via `f { a: 1 }`).
    let src = "fn f(x: Int) -> Int = x\n\n\nfn r(x: Int) -> { a: Int } = { a: x }\n\n\n\
               f({ a: 1 }.a)\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
    assert_eq!(
        toylang::run(src).unwrap(),
        toylang::run("fn f(x: Int) -> Int = x\n\n\nf(1)\n").unwrap()
    );
}

/// A call used as a postfix base is always parenthesized, even when its own argument would
/// otherwise print bare: `f(x)[i]` printed as `f x[i]` would reparse as `f(x[i])`, since a
/// bare argument is itself a postfix chain and the `[i]` would reattach there instead of to
/// the call's result. Found by the corpus's own behaviour check the day bare application
/// landed: nine real corpus programs changed behaviour this way before `print_atom_base` was
/// taught to always parenthesize a `Call` it is printing as a base.
#[test]
fn a_call_used_as_a_postfix_base_keeps_its_parens() {
    let src = "fn f(xs: Vec<Int>) -> Vec<Int> = xs\n\n\n[f([1, 2])[0]!, f([1, 2])[1]!]\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
    assert_eq!(toylang::run(src).unwrap(), "[1,2]\n");
}

/// A qualified variant's payload drops its parens exactly when it is a record literal
/// (maintainer ruling, 2026-09-19: the same class of gap bare application closed for calls).
/// `Shape.circle{r: 3}` is the only bare form the grammar's own payload production gives --
/// `ident_expr` recognises the payload on a bare `(` or `{` alone, with no `self.postfix()`
/// fallback the way a call's bare argument gets, so a non-record payload (`Shape.some2(5)`)
/// has no bare spelling to fall back to and keeps its parens.
#[test]
fn a_record_variant_payload_prints_bare() {
    let src = "enum Shape { Point, Circle{r: Int} }\n\n\n\
               fn area(s: Shape) -> Int = s | Circle{r} -> r * r or Point -> 0\n\n\n\
               area(Shape.circle({r: 3}))\n";
    let want = "enum Shape { Point, Circle { r: Int } }\n\n\n\
                fn area(s: Shape) -> Int = s | Circle { r } -> r * r or Point -> 0\n\n\n\
                area Shape.circle { r: 3 }\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
    assert_eq!(toylang::run(src).unwrap(), toylang::run(want).unwrap());

    // A non-record payload (Int, here) has no bare form at all -- `Shape.some2 5` does not
    // parse -- so it keeps its parens even though the *outer* call bare-applies around it.
    let src = "enum Opt2 { Some2(Int), None2 }\n\n\n\
               fn f(x: Opt2) -> Int = x | Some2 -> . or 0\n\n\n\
               f(Opt2.some2(5))\n";
    let want = "enum Opt2 { Some2(Int), None2 }\n\n\n\
                fn f(x: Opt2) -> Int = x | Some2 -> . or 0\n\n\n\
                f Opt2.some2(5)\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(want).unwrap(), want);
    assert_eq!(toylang::run(src).unwrap(), toylang::run(want).unwrap());
}

/// A record payload that does not fit compact breaks bare, one field per line, since the
/// grammar's bare-brace payload form has no wrapping parens to break inside of.
#[test]
fn a_long_record_variant_payload_breaks_bare() {
    let src = "enum S { C{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: Int, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: Int} }\n\nfn f(x: S) -> Int = x | C{aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb} -> aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa + bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb\n\nf(S.c({aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: 1, bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb: 2}))\n";
    let want = toylang::fmt(src).unwrap();
    assert!(want.contains("f(\n  S.c {\n"), "{want}");
    assert_eq!(toylang::fmt(&want).unwrap(), want);
    assert_eq!(toylang::run(src).unwrap(), toylang::run(&want).unwrap());
}

/// Bare application is capped at one hop (maintainer ruling, 2026-09-19): two or more names
/// chained bare with nothing between them (`foo bar 3`) reads right-to-left correctly by the
/// grammar's own rule, but a person has to hold that rule to do it, and it gets hard past one
/// hop. `foo(3)` is one hop and stays bare; two names in a row is where a call's own argument
/// stops counting as bare-safe.
///
/// A `Call` is safe as someone else's bare argument exactly when it would not itself render
/// bare: if it would (`bar 3`), placing it right after another bare name produces the run
/// this rule forbids (`foo bar 3`); if it renders parenthesized instead, because its own
/// argument failed this same check, its parens already delimit it and a bare name in front of
/// it is unambiguous (`foo bar(3)`). The result alternates parens and bare reading outward
/// from the innermost call, one hop at a time, rather than parenthesizing every level once a
/// chain reaches two.
#[test]
fn bare_application_is_capped_at_one_hop() {
    let h = "fn foo(x: Int) -> Int = x * 10\n\n\n\
             fn bar(x: Int) -> Int = x + 1\n\n\n\
             fn baz(x: Int) -> Int = x - 1\n\n\n\
             fn qux(x: Int) -> Int = x * 2\n\n\n";
    let cases = [
        ("foo(3)", "foo 3"),
        ("foo(bar(3))", "foo(bar 3)"),
        ("foo(bar(baz(3)))", "foo bar(baz 3)"),
        ("foo(bar(baz(qux(3))))", "foo(bar baz(qux 3))"),
    ];
    for (src_body, want_body) in cases {
        let src = format!("{h}{src_body}\n");
        let want = format!("{h}{want_body}\n");
        assert_eq!(toylang::fmt(&src).unwrap(), want, "formatting {src_body}");
        assert_eq!(
            toylang::fmt(&want).unwrap(),
            want,
            "{want_body} is not idempotent"
        );
        assert_eq!(
            toylang::run(&src).unwrap(),
            toylang::run(&want).unwrap(),
            "{src_body} changed behaviour"
        );
    }
}

/// A record argument that does not fit compact breaks bare, one field per line, the same way
/// a variant's record payload already does (maintainer finding, 2026-09-19, in a real
/// recursive function): the `{}` is already the call's own delimiter, so wrapping it in a
/// second, real pair of parens (`join_digits(\n  { ... }\n)`) added a pair the record's own
/// braces made redundant. The compact printer already rendered this bare when it fit; this is
/// that same choice carried into the wrapped form.
#[test]
fn a_long_record_call_argument_breaks_bare() {
    let src = "fn join_digits({ digits, acc }: { digits: Vec<Int>, acc: Int64 }) -> Int64 =\n  length digits == 0\n  | . -> acc or\n    join_digits({ digits: tail(digits)!, acc: acc * 10 + i64 digits[0]! })\n\njoin_digits({ digits: [1, 2, 3], acc: 0 })\n";
    let want = toylang::fmt(src).unwrap();
    assert!(
        want.contains("join_digits {\n      digits:"),
        "expected a bare, broken record argument:\n{want}"
    );
    assert_eq!(
        want.matches("join_digits(\n").count(),
        1,
        "only the signature should open with a wrapped paren, not the recursive call:\n{want}"
    );
    assert_eq!(toylang::fmt(&want).unwrap(), want);
    assert_eq!(toylang::run(src).unwrap(), toylang::run(&want).unwrap());
}

/// A `let` block's value is set apart from the bindings that feed it by a blank line
/// (maintainer ruling, 2026-09-19), the same way two blank lines now separate top-level
/// declarations: a visible boundary between the setup and the thing it computes.
#[test]
fn a_let_blocks_value_gets_a_blank_line_before_it() {
    let src = "fn f(x: Int) -> Int =\n  let a = x * 2\n  let b = a + 1\n  a + b\n\n\nf 1\n";
    let want = "fn f(x: Int) -> Int =\n  let a = x * 2\n  let b = a + 1\n\n  a + b\n\n\nf 1\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(&want).unwrap(), want);
    assert_eq!(toylang::run(src).unwrap(), toylang::run(&want).unwrap());
}

/// An impl method's body may be a `let` block, the same as a top-level function's --
/// `parse.rs::impl_decl` calls the same `def_body()` a top-level `fn` does. The formatter's
/// `let`-block path was only ever built for a top-level definition (`print_let_def`), so a
/// real, checker-accepted impl method with one crashed `fmt` with "a `let` block is only ever
/// a function body" until `print_impl_let_method` gave it the same treatment, one level deeper
/// since the method already sits inside the impl's own braces.
#[test]
fn an_impl_methods_let_body_formats_without_panicking() {
    let src = "trait M {\n  fn m(x: Int) -> Int\n}\n\n\nimpl M for Int {\n  fn m(x: Int) -> Int =\n    let a = x * 2\n    a + 1\n}\n\n\n1:m(3)\n";
    let want = "trait M {\n  fn m(x: Int) -> Int\n}\n\n\nimpl M for Int {\n  fn m(x: Int) -> Int =\n    let a = x * 2\n\n    a + 1\n}\n\n\n1:m(3)\n";
    assert_eq!(toylang::fmt(src).unwrap(), want);
    assert_eq!(toylang::fmt(&want).unwrap(), want);
    assert_eq!(toylang::run(src).unwrap(), toylang::run(&want).unwrap());
}

/// A bare call sitting right next to a binary operator, comparison, or guard has nothing
/// marking where its argument ends and the operator begins (maintainer ruling, 2026-09-20,
/// found in a real function: `(length_digits v - 1) * 8 + digit_count v[-1]!`) -- the same
/// ambiguity bare application was built to avoid, just against an operator instead of another
/// call. The call falls back to its own parenthesized form, with no extra wrap on top:
/// `length(v) == 0`, not `(length v) == 0`.
///
/// Exempted when the argument's own rendering already closes with `}` or `]`: a record
/// literal, or a postfix chain ending in an index/slice/projection, already marks its own end
/// as clearly as parens would, so `get { r: 3 } * get { r: 4 }` is left bare.
#[test]
fn a_bare_call_next_to_an_operator_falls_back_to_parens() {
    let h = "fn length_digits(v: Vec<Int>) -> Int = length v\n\n\n\
             fn digit_count(x: Int) -> Int = x - 5\n\n\n\
             fn get(p: { r: Int }) -> Int = p.r\n\n\n";
    let src = format!(
        "{h}fn digits_of(v: Vec<Int>) -> Int =\n\
         \x20 (length_digits v - 1) * 8 + digit_count v[-1]!\n\n\n\
         digits_of([100, 5])\n"
    );
    let want = format!(
        "{h}fn digits_of(v: Vec<Int>) -> Int =\n\
         \x20 (length_digits(v) - 1) * 8 + digit_count(v[-1]!)\n\n\n\
         digits_of([100, 5])\n"
    );
    assert_eq!(toylang::fmt(&src).unwrap(), want);
    assert_eq!(toylang::fmt(&want).unwrap(), want);
    assert_eq!(toylang::run(&src).unwrap(), toylang::run(&want).unwrap());

    // A record literal ends the argument's rendering with `}`, which already marks where it
    // stops, so no fallback is needed even though the call sits right next to `*`.
    let bracketed = format!("{h}get({{ r: 3 }}) * get({{ r: 4 }})\n");
    let bracketed_want = format!("{h}get {{ r: 3 }} * get {{ r: 4 }}\n");
    assert_eq!(toylang::fmt(&bracketed).unwrap(), bracketed_want);
    assert_eq!(toylang::fmt(&bracketed_want).unwrap(), bracketed_want);
    assert_eq!(
        toylang::run(&bracketed).unwrap(),
        toylang::run(&bracketed_want).unwrap()
    );
}
