//! The agreement harness.
//!
//! One corpus, every backend. This is the check a single-backend suite cannot express: two
//! backends producing different answers is a failure in its own right, separate from either of
//! them producing a wrong one. The step_*.rs suites keep their own job, which is pinning error
//! messages and emitted code; this is about behaviour, and behaviour is what has to be identical
//! across targets.

mod support;

use toylang::Backend;

#[test]
fn every_backend_agrees_and_is_right() {
    let cases = support::cases();
    assert!(!cases.is_empty(), "the corpus is empty, so this test proves nothing");

    let mut failures = Vec::new();

    for case in &cases {
        let mut outputs: Vec<(&str, String)> = Vec::new();
        for backend in Backend::ALL {
            // A backend that cannot run is reported, never skipped. A report saying every
            // backend agreed when only one of them ran is worse than no report.
            match toylang::run_on(&case.program, case.input.as_deref(), backend) {
                Ok(out) => outputs.push((backend.name(), out)),
                Err(e) => failures.push(format!(
                    "BROKEN  {}: {} could not run: {e}",
                    case.name,
                    backend.name()
                )),
            }
        }

        if outputs.len() < Backend::ALL.len() {
            continue;
        }

        // Disagreement first, and reported on its own. Which backend matches the expectation is
        // not the point: the language is underspecified either way.
        let (_, first) = &outputs[0];
        if outputs.iter().any(|(_, out)| out != first) {
            let shown: Vec<String> = outputs.iter().map(|(n, o)| format!("{n}={o:?}")).collect();
            failures.push(format!("DISAGREE {}: {}", case.name, shown.join(" ")));
            continue;
        }

        if *first != case.output {
            failures.push(format!(
                "WRONG   {}: expected {:?}, every backend gave {:?}",
                case.name, case.output, first
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} corpus programs failed across {} backends:\n{}",
        failures.len(),
        cases.len(),
        Backend::ALL.len(),
        failures.join("\n")
    );
}

/// What a program prints is not always the whole claim. A case that asks for it gets the code a
/// backend emitted pinned as well, which is how a property invisible in the output -- a name
/// that has to be declared, a type that has to be spelled -- stays observed.
#[test]
fn emitted_code_matches_the_snapshot() {
    let mut asked = 0;
    for case in support::cases() {
        let program = toylang::compile(&case.program)
            .unwrap_or_else(|e| panic!("{}: {e}", case.name));
        for backend in case.snapshot {
            let emitted = backend
                .emit(&program)
                .unwrap_or_else(|e| panic!("{}: {} could not emit: {e}", case.name, backend.name()));
            insta::assert_snapshot!(format!("{}__{}", case.name, backend.name()), emitted);
            asked += 1;
        }
    }
    assert!(asked > 0, "no case asks for a snapshot, so this test proves nothing");
}
