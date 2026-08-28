//! Tags every corpus case with the AST shapes it exercises.
//!
//! Runs unconditionally on every `cargo test`, the same way `export_site.rs` regenerates
//! `corpus.json`: the tags are derived from the compiled program, so keeping them fresh is
//! cheaper than a staleness check would be. The patch touches only the `node_types:` line,
//! never the rest of the file, so the comments the corpus authors wrote survive it.

mod support;

use std::path::Path;

#[test]
fn tag_the_corpus_with_node_types() {
    for case in support::cases() {
        let program =
            toylang::compile(&case.program).unwrap_or_else(|e| panic!("{}: {e}", case.name));
        let tags = toylang::tags::node_types(&program);
        let path = support::dir().join(format!("{}.yaml", case.name));
        patch_node_types(&path, &tags);
    }
}

fn patch_node_types(path: &Path, tags: &[String]) {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let line = format!("node_types: [{}]", tags.join(", "));

    let mut lines: Vec<&str> = text.lines().collect();
    let new_text = match lines.iter().position(|l| l.starts_with("node_types:")) {
        Some(i) => {
            lines[i] = &line;
            lines.join("\n")
        }
        None => format!("{}\n\n{line}", text.trim_end()),
    };

    let new_text = format!("{}\n", new_text.trim_end());
    if new_text != text {
        std::fs::write(path, new_text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}
