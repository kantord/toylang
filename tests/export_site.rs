//! Exports the corpus, and what every backend makes of it, for the browsable site.
//!
//! A test rather than a binary because it is one: the site is only worth publishing if every
//! case in it compiles on every target, and finding that out is the same walk as writing the
//! file. What it writes is committed, so the site builds without a Rust toolchain.

mod support;

use std::collections::BTreeMap;

use serde_json::{Value, json};
use support::Expect;
use toylang::Backend;

/// Written into the site's static assets rather than its source, so the payload is one cacheable
/// request instead of megabytes of emitted code inlined into the bundle.
const OUT: &str = "site/public/corpus.json";

#[test]
fn export_the_corpus_for_the_site() {
    let cases = support::cases();
    assert!(!cases.is_empty(), "nothing to export");

    let mut exported = Vec::new();
    let mut totals: BTreeMap<&str, usize> = BTreeMap::new();

    for case in &cases {
        let program =
            toylang::compile(&case.program).unwrap_or_else(|e| panic!("{}: {e}", case.name));

        let mut emitted = serde_json::Map::new();
        for backend in Backend::ALL {
            let code = backend.emit(&program).unwrap_or_else(|e| {
                panic!("{}: {} could not emit: {e}", case.name, backend.name())
            });
            *totals.entry(backend.name()).or_default() += code.len();
            emitted.insert(backend.name().to_string(), json!(code));
        }
        let expect = match &case.expect {
            Expect::Output(out) => json!({ "kind": "output", "value": out }),
            Expect::Refusal => json!({ "kind": "refusal" }),
        };

        exported.push(json!({
            "name": case.name,
            "program": case.program,
            "input": case.input,
            "inputType": program.input.as_ref().map(|t| t.to_string()),
            // A case's `input` is raw text either way -- an `input` program's declared JSON
            // type, or a `lines` program's fixture lines -- and the site needs to know which,
            // since only one of them is meant to be valid JSON.
            "usesLines": program.uses_lines,
            "resultType": program.body.ty.to_string(),
            "nodeTypes": toylang::tags::node_types(&program),
            "expect": expect,
            "emitted": Value::Object(emitted),
        }));
    }

    let payload = json!({
        "backends": Backend::ALL.iter().map(|b| b.name()).collect::<Vec<_>>(),
        "cases": exported,
    });

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(OUT);
    std::fs::create_dir_all(path.parent().expect("has a parent")).expect("site directory");
    let text = serde_json::to_string_pretty(&payload).expect("serialisable");
    std::fs::write(&path, format!("{text}\n")).expect("writable");

    let sizes: Vec<String> = totals
        .iter()
        .map(|(n, b)| format!("{n} {}k", b / 1024))
        .collect();
    println!("{} cases -> {} ({})", cases.len(), OUT, sizes.join(", "));
}
