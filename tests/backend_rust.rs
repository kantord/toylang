//! The Rust backend against the corpus.
//!
//! Rust joined `Backend::ALL` once it could compile the whole corpus, the same way native did.
//! `tests/corpus.rs` now covers it like any other backend; this file is what watched the gap
//! while it was still open, and stays as a snapshot of the "not yet" list -- empty for now, and
//! it should only ever grow back temporarily, never silently.

mod support;

use toylang::Backend;

#[test]
fn rust_agrees_where_it_compiles() {
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();

    for case in support::cases() {
        let (name, src, input) = (case.name, case.program, case.input);
        let program = toylang::compile(&src).expect("corpus programs compile");
        let source = toylang::emit_rs::emit(&program);

        let dir = tempfile::tempdir().expect("temp dir");
        let exe = dir.path().join("program");
        match toylang::link_rust(&source, &exe) {
            Err(reason) => unsupported.push(format!("{name}: {reason}")),
            Ok(()) => {
                // Compared as results rather than as output, so a case that every backend has
                // to refuse is checked here too: both refusing is agreement, and one refusing
                // while the other runs is the disagreement worth catching.
                let rust = toylang::run_on(&src, input.as_deref(), Backend::Rust);
                let lua = toylang::run_on(&src, input.as_deref(), Backend::Lua);
                match (rust, lua) {
                    (Ok(r), Ok(l)) => assert_eq!(r, l, "{name}: rust and lua disagree"),
                    (Err(_), Err(_)) => {}
                    (r, l) => panic!("{name}: rust gave {r:?} and lua gave {l:?}"),
                }
                supported.push(name);
            }
        }
    }

    assert!(
        !supported.is_empty(),
        "rust compiles nothing, so this test proves nothing"
    );

    insta::assert_snapshot!(format!(
        "compiles (rust) ({}):\n{}\n\nnot yet ({}):\n{}",
        supported.len(),
        supported.join("\n"),
        unsupported.len(),
        unsupported.join("\n")
    ));
}
