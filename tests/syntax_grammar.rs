//! The generated syntax-highlighting grammar (see `toylang::syntax_gen`) is checked into the
//! repo twice -- once for the docs site, once inside the VS Code extension folder -- so both
//! consumers work without a build step of their own. This test is what keeps either copy from
//! going stale: it regenerates the grammar in-memory and fails if either file on disk disagrees,
//! naming the fix (`cargo run --bin gen_syntax`) rather than just the mismatch.

use std::path::Path;

fn assert_matches_generated(path: &str, expected: &str) {
    let on_disk =
        std::fs::read_to_string(path).unwrap_or_else(|e| panic!("could not read {path}: {e}"));
    assert_eq!(
        on_disk, expected,
        "{path} is stale relative to toylang::syntax_gen::grammar() -- run `cargo run --bin gen_syntax` and commit the result"
    );
}

#[test]
fn generated_grammar_is_up_to_date_everywhere_it_is_checked_in() {
    let expected = format!(
        "{}\n",
        serde_json::to_string_pretty(&toylang::syntax_gen::grammar()).unwrap()
    );
    for path in [
        "syntax/toylang.tmLanguage.json",
        "editors/vscode/syntaxes/toylang.tmLanguage.json",
    ] {
        assert!(
            Path::new(path).exists(),
            "{path} does not exist -- run `cargo run --bin gen_syntax`"
        );
        assert_matches_generated(path, &expected);
    }
}
