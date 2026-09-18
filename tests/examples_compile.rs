//! Every `examples/*.toy` file compiles. The README links these as what the language looks
//! like, and until this test only the formatting sweep read them: `fizzbuzz.toy` sat with a
//! type error for days (`join_lines` over a `Stream`, once `range` became a source) because
//! a formatter does not type-check.

use std::path::Path;

#[test]
fn every_example_compiles() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("examples");
    let mut checked = 0;
    let mut failures = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("examples/ exists") {
        let path = entry.expect("readable entry").path();
        if path.extension().is_none_or(|e| e != "toy") {
            continue;
        }
        let src = std::fs::read_to_string(&path).expect("readable example");
        checked += 1;
        if let Err(e) = toylang::compile_in(&src, &dir) {
            failures.push(format!("{}: {e}", path.display()));
        }
    }
    assert!(checked > 0, "no examples found, so this test proves nothing");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
