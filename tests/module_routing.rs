//! `@(path)` module-as-function routing (gh:167): the syntax parses, but resolution,
//! acceptance, and codegen do not exist yet, so the checker refuses it cleanly rather than
//! panicking or attempting to unify/dispatch. This file pins that refusal; the semantics row
//! (`module-routing-semantics-build`) is where the construct starts doing anything.

/// A `@(path)` arm parses now but is not yet supported, so compiling one is refused with the
/// row's pinned message instead of reaching codegen's AST-only stub.
#[test]
fn module_route_not_yet_supported() {
    insta::assert_snapshot!(
        toylang::compile(r#"@("some/path")"#)
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}
