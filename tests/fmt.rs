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

/// The one piece of source text `emit_toylang::emit` cannot reconstruct from the parsed tree --
/// see its module doc -- is a leading comment banner, which `fmt` reattaches from the raw text
/// instead. Pinned directly since nothing else here exercises it: every corpus program is
/// comment-free.
#[test]
fn a_leading_comment_survives_formatting() {
    let src = "# Keep the elements that are at least 2.\n[1, 2, 3] | select(. >= 2)\n";
    assert_eq!(toylang::fmt(src).unwrap(), src);
}
