//! The native backend against the corpus.
//!
//! Native is not in `Backend::ALL` yet, because it cannot compile the whole language and a
//! partial backend would turn the agreement harness permanently red. That absence would be a
//! silent skip if nothing else watched it, so this file watches it: every corpus program is
//! either compiled natively and checked against Lua, or listed by name in a snapshot of what
//! native cannot do. The snapshot has to shrink at steps 5 and 6, and cannot quietly grow.

mod support;

use toylang::Backend;

/// Everything native compiles must agree with Lua. Everything it does not is named here with
/// the reason, so the gap is a tracked artifact rather than an absence.
#[test]
fn native_agrees_where_it_compiles() {
    let mut supported = Vec::new();
    let mut unsupported = Vec::new();

    for case in support::cases() {
        let (name, src, input) = (case.name, case.program, case.input);
        let program = toylang::compile(&src).expect("corpus programs compile");
        match toylang::emit_llvm::to_ir(&program) {
            Err(reason) => unsupported.push(format!("{name}: {reason}")),
            Ok(_) => {
                // Compared as results rather than as output, so a case that every backend has
                // to refuse is checked here too: both refusing is agreement, and one refusing
                // while the other runs is the disagreement worth catching.
                let native = toylang::run_on(&src, input.as_deref(), Backend::Native);
                let lua = toylang::run_on(&src, input.as_deref(), Backend::Lua);
                match (native, lua) {
                    (Ok(n), Ok(l)) => assert_eq!(n, l, "{name}: native and lua disagree"),
                    (Err(_), Err(_)) => {}
                    (n, l) => panic!("{name}: native gave {n:?} and lua gave {l:?}"),
                }
                supported.push(name);
            }
        }
    }

    assert!(
        !supported.is_empty(),
        "native compiles nothing, so this test proves nothing"
    );

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
