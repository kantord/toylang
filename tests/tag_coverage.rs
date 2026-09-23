//! Every AST shape the tagger can name has a corpus case that exercises it, or a named
//! reason why not. The corpus is the spec (ADR 0003), so a `tir::Kind` with no case is a
//! feature the spec does not mention: Float ran on seven backends for twelve days with no
//! corpus case and no reference page before anyone noticed, because the only completeness
//! gate covered builtin names. This one covers the tag vocabulary, in both directions -- a
//! tag a case uses that `tags::TAGS` does not list is a new shape that was never added to
//! the list, and a listed tag no case uses is a gap that needs a row.

use std::collections::BTreeSet;

mod support;

/// Tags with no corpus case yet, each with the board row that will add one. An entry here
/// is a debt, not an exemption: it comes out when the row lands.
const UNCOVERED: &[(&str, &str)] = &[
    (
        "sort-by",
        "sort-by-max-by-jq, the last of the per-backend rows",
    ),
    (
        "max-by",
        "sort-by-max-by-jq, the last of the per-backend rows",
    ),
    (
        "builtin.pipe_through",
        "no corpus case: pipe_through runs only on a subset of backends (go, rust, py, js, lua) and reads a subprocess",
    ),
    (
        "closure",
        "no corpus case: closures-first-class-functions-design lands closures on Rust, Go and JS \
only so far (2026-09-23); the other four backends have no representation for a stored closure \
value yet, so no case can run on all seven",
    ),
    (
        "closure.apply",
        "no corpus case: same reason as `closure` above -- applying one only exists where the \
closure itself does, Rust, Go and JS only for now",
    ),
];

fn used_tags() -> BTreeSet<String> {
    let mut used = BTreeSet::new();
    for case in support::cases() {
        let path = support::dir().join(format!("{}.yaml", case.name));
        let text =
            std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let Some(line) = text.lines().find(|l| l.starts_with("node_types:")) else {
            continue;
        };
        let inner = line
            .trim_start_matches("node_types:")
            .trim()
            .trim_start_matches('[')
            .trim_end_matches(']');
        for tag in inner.split(',').map(str::trim).filter(|t| !t.is_empty()) {
            used.insert(tag.to_string());
        }
    }
    used
}

#[test]
fn every_tag_has_a_corpus_case_or_a_named_gap() {
    let used = used_tags();
    assert!(
        !used.is_empty(),
        "no corpus case carries node_types; run tag_corpus first"
    );
    let listed: BTreeSet<&str> = toylang::tags::TAGS.iter().copied().collect();

    let unlisted: Vec<&String> = used
        .iter()
        .filter(|t| !listed.contains(t.as_str()))
        .collect();
    assert!(
        unlisted.is_empty(),
        "tags the corpus uses that tags::TAGS does not list: {unlisted:?}"
    );

    let mut failures = Vec::new();
    for tag in &listed {
        let covered = used.contains(*tag);
        let excused = UNCOVERED.iter().find(|(t, _)| t == tag);
        match (covered, excused) {
            (true, None) | (false, Some(_)) => {}
            (false, None) => failures.push(format!(
                "`{tag}` has no corpus case and no entry in UNCOVERED"
            )),
            (true, Some((_, row))) => failures.push(format!(
                "`{tag}` now has a corpus case; drop it from UNCOVERED ({row})"
            )),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
