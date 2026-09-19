//! The sweep harness: every piece of toylang source the repository holds is required to
//! already be in `toylang fmt`'s canonical form (maintainer ruling, 2026-09-19: enforce
//! everywhere), so a hand-edited example cannot drift from what a real user would get by
//! running the formatter on it. Four kinds of holder:
//!
//! - every `.toy` file the project-wide walk reaches (`fmt_tree::run` from the repo root:
//!   examples/, benches/, tests/modules/, prelude.toy);
//! - every `toylang` or `toy` fence (any trailing words, so `toylang slow` too) under docs/,
//!   plans/, the README, and draft.md;
//! - every corpus case's `program`.
//!
//! One escape, narrow on purpose: a fragment that exists to show a spelling the canonical
//! style does not use -- bare application, the brace-call shorthand, the `csv` sugar -- opens
//! with the exact line `# fmt: syntax-example`, checked for as plain text before parsing (a
//! marker, not a directive `fmt` itself understands); reformatting it would erase the very
//! thing the surrounding prose or test is pointing at.
//!
//! A fence that does not parse is a claim about the parser, not about formatting. Under docs/
//! and in the README it must be followed by an `error` fence, which is how the docs harness
//! proves the claim, or carry the marker; anything else is a broken example. Under plans/ a
//! fence that does not parse is skipped: those pages sketch syntax that never landed.

mod support;

use std::path::{Path, PathBuf};

/// Plain text, not a `fmt`-recognized directive: this file is the only reader of it.
const EXEMPT_MARKER: &str = "# fmt: syntax-example";

#[test]
fn every_toy_file_the_walk_reaches_is_already_formatted() {
    let report = toylang::fmt_tree::run(&repo_root(), toylang::fmt_tree::Mode::Check);
    let mut failures: Vec<String> = report
        .changed
        .iter()
        .map(|p| format!("{}: not in canonical form -- run `toylang fmt --write`", p.display()))
        .collect();
    failures.extend(
        report
            .failed
            .iter()
            .map(|(p, e)| format!("{}: {e}", p.display())),
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn every_docs_fragment_is_already_formatted() {
    let mut checked = 0;
    let mut failures = Vec::new();

    let mut pages = Vec::new();
    walk(&repo_root().join("docs"), &mut pages);
    walk(&repo_root().join("plans"), &mut pages);
    pages.push(repo_root().join("README.md"));
    pages.push(repo_root().join("draft.md"));
    pages.sort();

    for page in pages {
        let text =
            std::fs::read_to_string(&page).unwrap_or_else(|e| panic!("{}: {e}", page.display()));
        let rel = page
            .strip_prefix(repo_root())
            .expect("under the repo")
            .to_string_lossy()
            .into_owned();
        let sketches_allowed = rel.starts_with("plans/");
        for fence in toylang_fences(&text) {
            checked += 1;
            check_fence(&format!("{rel}:{}", fence.line), &fence, sketches_allowed, &mut failures);
        }
    }

    assert!(
        checked > 0,
        "found no toylang fences under docs/, so this test proves nothing"
    );
    report(checked, &failures);
}

#[test]
fn every_corpus_program_is_already_formatted() {
    let cases = support::cases();
    let mut failures = Vec::new();
    for case in &cases {
        if case.program.trim_start().starts_with(EXEMPT_MARKER) {
            continue;
        }
        let formatted = match toylang::fmt(&case.program) {
            Ok(formatted) => formatted,
            Err(e) => {
                failures.push(format!("{}: does not parse: {e}", case.name));
                continue;
            }
        };
        if formatted != case.program {
            failures.push(format!(
                "{}: program is not in canonical form -- run `toylang fmt` on it, or open it \
                 with `{EXEMPT_MARKER}` if it deliberately shows a non-canonical spelling\n\
                 --- as written ---\n{}--- canonical ---\n{formatted}",
                case.name, case.program
            ));
        }
    }
    report(cases.len(), &failures);
}

struct Fence {
    line: usize,
    body: String,
    /// Whether an `error` fence follows: the docs harness's way of saying the program is
    /// expected not to compile, which covers not parsing.
    followed_by_error: bool,
}

fn check_fence(at: &str, fence: &Fence, sketches_allowed: bool, failures: &mut Vec<String>) {
    let src = &fence.body;
    if src.trim_start().starts_with(EXEMPT_MARKER) {
        return;
    }
    let formatted = match toylang::fmt(src) {
        Ok(formatted) => formatted,
        Err(_) if fence.followed_by_error || sketches_allowed => return,
        Err(e) => {
            failures.push(format!(
                "{at}: does not parse ({e}) and no `error` fence follows -- fix it, pair it \
                 with an `error` fence, or mark it `{EXEMPT_MARKER}`"
            ));
            return;
        }
    };
    if formatted != *src {
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

/// Whether a fence's info string names toylang source: `toylang`, `toy`, with any trailing
/// words (`toylang slow`).
fn is_toylang_fence(info: &str) -> bool {
    matches!(info.split_whitespace().next(), Some("toylang" | "toy"))
}

/// Every toylang fence in a markdown page, with the line its body starts on and whether the
/// next fence on the page is an `error` fence. Lighter than `tests/docs.rs`'s `extract`: this
/// only needs the program text, not the input/output fences that go with it, since formatting
/// does not care what a program prints.
fn toylang_fences(text: &str) -> Vec<Fence> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out: Vec<Fence> = Vec::new();
    // Whether the fence read last was a toylang one: an `error` fence right after it is its.
    let mut previous_is_toylang = false;
    let mut i = 0;
    while i < lines.len() {
        let Some(info) = lines[i].trim().strip_prefix("```") else {
            i += 1;
            continue;
        };
        if info.trim().is_empty() {
            i += 1;
            continue;
        }
        let start = i + 2;
        let mut body = String::new();
        i += 1;
        while i < lines.len() && lines[i].trim() != "```" {
            body.push_str(lines[i]);
            body.push('\n');
            i += 1;
        }
        i += 1;
        if info.trim() == "error"
            && previous_is_toylang
            && let Some(last) = out.last_mut()
        {
            last.followed_by_error = true;
        }
        previous_is_toylang = is_toylang_fence(info);
        if previous_is_toylang {
            out.push(Fence {
                line: start,
                body,
                followed_by_error: false,
            });
        }
    }
    out
}
