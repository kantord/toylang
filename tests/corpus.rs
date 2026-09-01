//! The agreement harness.
//!
//! One corpus, every backend. This is the check a single-backend suite cannot express: two
//! backends producing different answers is a failure in its own right, separate from either of
//! them producing a wrong one. The step_*.rs suites keep their own job, which is pinning error
//! messages and emitted code; this is about behaviour, and behaviour is what has to be identical
//! across targets.

mod support;

/// The agreement check for one slice of the corpus: every `of`th case by index. Splitting by
/// index, not by backend, is the point -- `agreement_failures` runs every backend internally, so
/// the case is the unit of work a parallel runner can hand to a thread, and each shard keeps the
/// same bar a single test always had.
fn assert_shard_agrees(shard: usize, of: usize) {
    let cases = support::cases();
    assert!(
        !cases.is_empty(),
        "the corpus is empty, so this test proves nothing"
    );

    let mut failures = Vec::new();

    for (i, case) in cases.iter().enumerate() {
        if i % of != shard {
            continue;
        }
        failures.extend(support::agreement_failures(
            &case.name,
            &case.program,
            case.input.as_deref(),
            &case.expect,
        ));
    }

    assert!(
        failures.is_empty(),
        "{} corpus programs failed in shard {shard}/{of}:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn every_backend_agrees_and_is_right_shard_0() {
    assert_shard_agrees(0, 8);
}
#[test]
fn every_backend_agrees_and_is_right_shard_1() {
    assert_shard_agrees(1, 8);
}
#[test]
fn every_backend_agrees_and_is_right_shard_2() {
    assert_shard_agrees(2, 8);
}
#[test]
fn every_backend_agrees_and_is_right_shard_3() {
    assert_shard_agrees(3, 8);
}
#[test]
fn every_backend_agrees_and_is_right_shard_4() {
    assert_shard_agrees(4, 8);
}
#[test]
fn every_backend_agrees_and_is_right_shard_5() {
    assert_shard_agrees(5, 8);
}
#[test]
fn every_backend_agrees_and_is_right_shard_6() {
    assert_shard_agrees(6, 8);
}
#[test]
fn every_backend_agrees_and_is_right_shard_7() {
    assert_shard_agrees(7, 8);
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
