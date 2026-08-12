//! The agreement harness.
//!
//! One corpus, every backend. This is the check a single-backend suite cannot express: two
//! backends producing different answers is a failure in its own right, separate from either of
//! them producing a wrong one. The step_*.rs suites keep their own job, which is pinning error
//! messages and emitted code; this is about behaviour, and behaviour is what has to be identical
//! across targets.

use std::path::{Path, PathBuf};

use toylang::Backend;

struct Case {
    name: String,
    src: String,
    input: Option<String>,
    expected: String,
}

fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

fn cases() -> Vec<Case> {
    let dir = corpus_dir();
    let mut entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "toy"))
        .collect();
    entries.sort();

    entries
        .into_iter()
        .map(|path| {
            let name = path.file_stem().expect("has a stem").to_string_lossy().into_owned();
            let expected_path = path.with_extension("out");
            // A program with no expectation is a failure, not something to pass over.
            let expected = std::fs::read_to_string(&expected_path).unwrap_or_else(|_| {
                panic!("{name}: no expected output at {}", expected_path.display())
            });
            let input_path = dir.join(format!("{name}.in.json"));
            let input = input_path.exists().then(|| {
                std::fs::read_to_string(&input_path).expect("readable input")
            });
            Case { name, src: std::fs::read_to_string(&path).expect("readable program"), input, expected }
        })
        .collect()
}

#[test]
fn every_backend_agrees_and_is_right() {
    let cases = cases();
    assert!(!cases.is_empty(), "the corpus is empty, so this test proves nothing");

    let mut failures = Vec::new();

    for case in &cases {
        let mut outputs: Vec<(&str, String)> = Vec::new();
        for backend in Backend::ALL {
            // A backend that cannot run is reported, never skipped. A report saying every
            // backend agreed when only one of them ran is worse than no report.
            match toylang::run_on(&case.src, case.input.as_deref(), backend) {
                Ok(out) => outputs.push((backend.name(), out)),
                Err(e) => failures.push(format!("BROKEN  {}: {} could not run: {e}", case.name, backend.name())),
            }
        }

        if outputs.len() < Backend::ALL.len() {
            continue;
        }

        // Disagreement first, and reported on its own. Which backend matches the expectation is
        // not the point: the language is underspecified either way.
        let (_, first) = &outputs[0];
        if outputs.iter().any(|(_, out)| out != first) {
            let shown: Vec<String> =
                outputs.iter().map(|(n, o)| format!("{n}={o:?}")).collect();
            failures.push(format!("DISAGREE {}: {}", case.name, shown.join(" ")));
            continue;
        }

        if *first != case.expected {
            failures.push(format!(
                "WRONG   {}: expected {:?}, every backend gave {:?}",
                case.name, case.expected, first
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
