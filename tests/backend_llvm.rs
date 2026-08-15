//! The native backend against the corpus.
//!
//! Native is not in `Backend::ALL` yet, because it cannot compile the whole language and a
//! partial backend would turn the agreement harness permanently red. That absence would be a
//! silent skip if nothing else watched it, so this file watches it: every corpus program is
//! either compiled natively and checked against Lua, or listed by name in a snapshot of what
//! native cannot do. The snapshot has to shrink at steps 5 and 6, and cannot quietly grow.

use std::path::{Path, PathBuf};

use toylang::Backend;

fn corpus() -> Vec<(String, String, Option<String>)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .expect("corpus directory")
        .map(|e| e.expect("entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "toy"))
        .collect();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let name = path.file_stem().expect("stem").to_string_lossy().into_owned();
            let src = std::fs::read_to_string(&path).expect("program");
            let input_path = dir.join(format!("{name}.in.json"));
            let input = input_path
                .exists()
                .then(|| std::fs::read_to_string(&input_path).expect("input"));
            (name, src, input)
        })
        .collect()
}

/// Everything native compiles must agree with Lua. Everything it does not is named here with
/// the reason, so the gap is a tracked artifact rather than an absence.
#[test]
fn native_agrees_where_it_compiles() {
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();

    for (name, src, input) in corpus() {
        let program = toylang::compile(&src).expect("corpus programs compile");
        match toylang::emit_llvm::to_ir(&program) {
            Err(reason) => unsupported.push(format!("{name}: {reason}")),
            Ok(_) => {
                let native = toylang::run_on(&src, input.as_deref(), Backend::Native)
                    .unwrap_or_else(|e| panic!("{name}: native run failed: {e}"));
                let lua = toylang::run_on(&src, input.as_deref(), Backend::Lua)
                    .unwrap_or_else(|e| panic!("{name}: lua run failed: {e}"));
                assert_eq!(native, lua, "{name}: native and lua disagree");
                supported.push(name);
            }
        }
    }

    assert!(!supported.is_empty(), "native compiles nothing, so this test proves nothing");

    insta::assert_snapshot!(format!(
        "compiles natively ({}):\n{}\n\nnot yet ({}):\n{}",
        supported.len(),
        supported.join("\n"),
        unsupported.len(),
        unsupported.join("\n")
    ));
}

#[test]
fn emitted_llvm_ir() {
    let program = toylang::compile(r#""hello world""#).unwrap();
    insta::assert_snapshot!(toylang::emit_llvm::to_ir(&program).unwrap());
}
