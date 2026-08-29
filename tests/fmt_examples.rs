//! The sweep harness: every `examples/*.toy` file, and every `toylang` fence under `docs/`, is
//! required to already be in `toylang fmt`'s canonical form -- checked here so a hand-edited
//! example cannot drift from what a real user would get by running the formatter on it.
//!
//! Two escapes, both narrow on purpose. A fragment that does not parse at all (`docs/tutorial/
//! 06-matching.md`'s deliberately malformed default-arm example) has nothing for `fmt` to
//! canonicalize, so it is skipped rather than failed. A fragment that exists specifically to
//! show a spelling the canonical style does not use -- bare application, the brace-call
//! shorthand -- opens with the exact line `# fmt: syntax-example`, checked for as plain text
//! before parsing (a marker, not a directive `fmt` itself understands); reformatting it would
//! erase the very thing the surrounding prose is pointing at. As of this writing there are three:
//! `docs/reference/syntax/functions.md`, `docs/tutorial/02-records.md`, and
//! `docs/tutorial/04-enums.md`.

use std::path::{Path, PathBuf};

/// Plain text, not a `fmt`-recognized directive: this file is the only reader of it.
const EXEMPT_MARKER: &str = "# fmt: syntax-example";

#[test]
fn every_example_file_is_already_formatted() {
    let mut checked = 0;
    let mut failures = Vec::new();

    let dir = repo_root().join("examples");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "toy"))
        .collect();
    paths.sort();

    for path in paths {
        let src =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        checked += 1;
        check_formatted(&path.display().to_string(), &src, &mut failures);
    }

    assert!(
        checked > 0,
        "found no examples/*.toy files, so this test proves nothing"
    );
    report(checked, &failures);
}

#[test]
fn every_docs_fragment_is_already_formatted() {
    let mut checked = 0;
    let mut failures = Vec::new();

    let mut pages = Vec::new();
    walk(&repo_root().join("docs"), &mut pages);
    pages.sort();

    for page in pages {
        let text =
            std::fs::read_to_string(&page).unwrap_or_else(|e| panic!("{}: {e}", page.display()));
        let rel = page
            .strip_prefix(repo_root())
            .expect("under the repo")
            .to_string_lossy()
            .into_owned();
        for (line, body) in toylang_fences(&text) {
            checked += 1;
            check_formatted(&format!("{rel}:{line}"), &body, &mut failures);
        }
    }

    assert!(
        checked > 0,
        "found no toylang fences under docs/, so this test proves nothing"
    );
    report(checked, &failures);
}

fn check_formatted(at: &str, src: &str, failures: &mut Vec<String>) {
    if src.trim_start().starts_with(EXEMPT_MARKER) {
        return;
    }
    // A fragment that cannot parse has nothing for `fmt` to canonicalize; that is a claim about
    // the checker (pinned elsewhere, in the docs harness), not about formatting.
    let Ok(formatted) = toylang::fmt(src) else {
        return;
    };
    if formatted != src {
        failures.push(format!(
            "{at}: not in canonical form -- run `toylang fmt` on it, or mark it \
             `{EXEMPT_MARKER}` if it deliberately shows a non-canonical spelling\n\
             --- as written ---\n{src}--- canonical ---\n{formatted}"
        ));
    }
}

fn report(checked: usize, failures: &[String]) {
    assert!(
        failures.is_empty(),
        "{} of {checked} not formatted:\n{}",
        failures.len(),
        failures.join("\n\n")
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in
        std::fs::read_dir(dir).unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
    {
        let path = entry.expect("readable entry").path();
        if path.is_dir() {
            walk(&path, out);
        } else if path.extension().is_some_and(|e| e == "md") {
            out.push(path);
        }
    }
}

/// Every `toylang`-fenced block in a markdown page, as (the line its body starts on, the body).
/// Lighter than `tests/docs.rs`'s `extract`: this only needs the program text, not the
/// input/output fences that go with it, since formatting does not care what a program prints.
fn toylang_fences(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut lines = text.lines().enumerate();
    while let Some((i, line)) = lines.next() {
        if line.trim() != "```toylang" {
            continue;
        }
        let start = i + 2;
        let mut body = String::new();
        for (_, l) in lines.by_ref() {
            if l == "```" {
                break;
            }
            body.push_str(l);
            body.push('\n');
        }
        out.push((start, body));
    }
    out
}
