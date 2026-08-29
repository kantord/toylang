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
    assert!(
        !cases.is_empty(),
        "the corpus is empty, so this test proves nothing"
    );

    let mut failures = Vec::new();

    for case in &cases {
        failures.extend(support::agreement_failures(
            &case.name,
            &case.program,
            case.input.as_deref(),
            &case.expect,
        ));
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
        let program =
            toylang::compile(&case.program).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        for backend in case.snapshot {
            let emitted = backend.emit(&program).unwrap_or_else(|e| {
                panic!("{}: {} could not emit: {e}", case.name, backend.name())
            });
            insta::assert_snapshot!(format!("{}__{}", case.name, backend.name()), emitted);
            asked += 1;
        }
    }
    assert!(
        asked > 0,
        "no case asks for a snapshot, so this test proves nothing"
    );
}
