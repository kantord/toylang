//! Every `benches/programs/*.toy` compiles and emits on every backend. `just bench` is the only
//! thing that runs these, and it is not in `just check`, so a language rule that turned
//! fannkuch-redux illegal (a trait method may not share a plain function's name) went unseen
//! until someone recorded a baseline.
//!
//! Emit only, never run: the front end and the seven emitters are where a language change
//! breaks a program. What the programs print is pinned by their tests/corpus twins, which run
//! everywhere; running them here at benchmark sizes would cost minutes for no extra coverage.

use std::path::Path;
use toylang::Backend;

#[test]
fn every_bench_program_emits_on_every_backend() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("benches/programs");
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("benches/programs exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|e| e != "toy") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("readable bench program");
        checked += 1;
        let program = match toylang::compile_in(&src, &dir) {
            Ok(p) => p,
            Err(e) => {
                failures.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        for backend in Backend::ALL {
            if let Err(e) = backend.emit(&program) {
                failures.push(format!("{} on {}: {e}", path.display(), backend.name()));
            }
        }
    }
    assert!(
        checked > 0,
        "no bench programs found, so this test proves nothing"
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
