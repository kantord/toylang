//! The docs harness: every code fragment in `docs/**/*.md` is a real program.
//!
//! A fragment is a `toylang` fence, an `input` fence if the program reads one, and exactly one
//! of an `output`, `refuses`, or `error` fence. The first two go through the same seven-backend
//! agreement check as the corpus, because a docs fragment is a corpus case defined in prose; an
//! `error` fence pins the checker's message for a program the docs show being turned away. A
//! page can also embed an existing corpus case by id with a `case` fence instead of repeating
//! its program.
//!
//! What this buys is that the documentation cannot lie: a claim about what a program prints is
//! run, not proofread, and drift between the docs and the compiler fails `just test`.

mod support;

use std::path::{Path, PathBuf};

use support::Expect;

/// What a fragment claims happens to its program.
enum Outcome {
    /// Compiles, runs on every backend, and they all print this.
    Output(String),
    /// Compiles, and every backend refuses to run it. The fence is empty: what each backend
    /// says while refusing is its own business, so there is no text to pin.
    Refusal,
    /// Does not compile, and this is the checker's message, verbatim.
    Error(String),
}

struct Fragment {
    /// `page:line` of the opening fence, so a failure points at the prose to fix.
    at: String,
    program: String,
    input: Option<String>,
    outcome: Outcome,
}

#[test]
fn every_fragment_is_a_real_program() {
    let mut fragments = Vec::new();
    let mut embedded = Vec::new();
    for (page, text) in pages() {
        extract(&page, &text, &mut fragments, &mut embedded);
    }
    assert!(
        !fragments.is_empty(),
        "the docs have no fragments, so this test proves nothing"
    );

    let mut failures = Vec::new();

    for f in &fragments {
        match &f.outcome {
            Outcome::Output(want) => failures.extend(support::agreement_failures(
                &f.at,
                &f.program,
                f.input.as_deref(),
                &Expect::Output(want.clone()),
            )),
            Outcome::Refusal => {
                // Compiling is checked first and separately: to `agreement_failures` a program
                // that does not compile looks exactly like one every backend refused.
                if let Err(e) = toylang::compile(&f.program) {
                    failures.push(format!("BROKEN  {}: does not compile: {e}", f.at));
                } else {
                    failures.extend(support::agreement_failures(
                        &f.at,
                        &f.program,
                        f.input.as_deref(),
                        &Expect::Refusal,
                    ));
                }
            }
            Outcome::Error(want) => match toylang::compile(&f.program) {
                Ok(_) => failures.push(format!(
                    "COMPILED {}: claims `{want}` but the program compiles",
                    f.at
                )),
                Err(e) if e.to_string() != *want => failures.push(format!(
                    "MISQUOTED {}: the checker says {:?}, the page says {want:?}",
                    f.at,
                    e.to_string()
                )),
                Err(_) => {}
            },
        }
    }

    for (at, id) in &embedded {
        if !support::dir().join(format!("{id}.yaml")).is_file() {
            failures.push(format!(
                "MISSING {at}: embeds corpus case `{id}`, which does not exist"
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} docs fragments failed:\n{}",
        failures.len(),
        fragments.len(),
        failures.join("\n")
    );
}

/// The reference is complete for everything implemented, checkably: a builtin without a page
/// fails here, the same way a corpus case without fresh tags fails tag_corpus.
#[test]
fn every_builtin_has_a_reference_page() {
    let missing: Vec<&str> = toylang::check::BUILTIN_NAMES
        .into_iter()
        .filter(|name| {
            !docs_dir()
                .join("reference/builtins")
                .join(format!("{name}.md"))
                .is_file()
        })
        .collect();
    assert!(
        missing.is_empty(),
        "builtins without a reference page under docs/reference/builtins/: {}",
        missing.join(", ")
    );
}

fn docs_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("docs")
}

/// Every markdown page under `docs/`, as (path relative to the repo, content), sorted so
/// failures come out in a stable order.
fn pages() -> Vec<(String, String)> {
    let mut paths = Vec::new();
    walk(&docs_dir(), &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(|p| {
            let text =
                std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            let rel = p
                .strip_prefix(Path::new(env!("CARGO_MANIFEST_DIR")))
                .expect("under the repo")
                .to_string_lossy()
                .into_owned();
            (rel, text)
        })
        .collect()
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

/// Pulls the fragments out of one page. Malformed structure -- an `input` with no program, a
/// program left without an expectation -- is a panic naming the line, like a malformed corpus
/// case: a fence that asks for nothing looks exactly like a fence that passes.
fn extract(
    page: &str,
    text: &str,
    fragments: &mut Vec<Fragment>,
    embedded: &mut Vec<(String, String)>,
) {
    // A `toylang` fence whose expectation fence has not arrived yet.
    let mut pending: Option<(String, String, Option<String>)> = None;
    let mut lines = text.lines().enumerate().peekable();

    while let Some((i, line)) = lines.next() {
        let Some(info) = line.strip_prefix("```") else {
            continue;
        };
        let at = format!("{page}:{}", i + 1);
        let mut body = String::new();
        loop {
            match lines.next() {
                Some((_, "```")) => break,
                Some((_, l)) => {
                    body.push_str(l);
                    body.push('\n');
                }
                None => panic!("{at}: fence never closes"),
            }
        }
        match info.trim() {
            "toylang" => {
                if let Some((prev, ..)) = pending {
                    panic!("{prev}: fragment has no `output`, `refuses`, or `error` fence");
                }
                pending = Some((at, body, None));
            }
            "input" => match &mut pending {
                Some((_, _, input @ None)) => *input = Some(body),
                Some((prev, _, Some(_))) => panic!("{at}: fragment at {prev} already has an input"),
                None => panic!("{at}: `input` fence with no `toylang` fence before it"),
            },
            "output" | "refuses" | "error" => {
                let Some((frag_at, program, input)) = pending.take() else {
                    panic!(
                        "{at}: `{}` fence with no `toylang` fence before it",
                        info.trim()
                    )
                };
                let outcome = match info.trim() {
                    "output" => Outcome::Output(body),
                    "error" => Outcome::Error(body.trim_end().to_string()),
                    _ => {
                        assert!(
                            body.is_empty(),
                            "{at}: a `refuses` fence is empty; what each backend says while \
                             refusing is its own business"
                        );
                        Outcome::Refusal
                    }
                };
                fragments.push(Fragment {
                    at: frag_at,
                    program,
                    input,
                    outcome,
                });
            }
            "case" => {
                let id = body.trim();
                assert!(
                    !id.is_empty() && !id.contains(char::is_whitespace),
                    "{at}: a `case` fence holds one corpus case id"
                );
                embedded.push((at, id.to_string()));
            }
            // Any other language is ordinary illustration, not a claim about the compiler.
            _ => {}
        }
    }
    if let Some((prev, ..)) = pending {
        panic!("{prev}: fragment has no `output`, `refuses`, or `error` fence");
    }
}
