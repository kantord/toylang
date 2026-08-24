//! Loading the corpus.
//!
//! One case per file. A program, the input it reads, the output it must produce and whatever
//! else it asks for are one thing, and were three files that only a shared stem tied together:
//! a program whose `.out` was missing was a panic at load, and a `.in.json` whose stem was
//! misspelt was silently a program that reads no input.
//!
//! Read by both the agreement harness and the native backend's tracker, which were two copies
//! of the same directory walk.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// The file as written. `deny_unknown_fields` is the point of the struct: a misspelt key would
/// otherwise be a case that quietly asks for nothing extra, which looks exactly like a case that
/// passes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fields {
    program: String,
    #[serde(default)]
    input: Option<String>,
    output: String,
    /// Backends whose emitted code is snapshotted for this program, for cases where what the
    /// program prints is not the whole claim. Empty for most cases: running on every backend and
    /// agreeing is the ordinary bar, and a snapshot on top of it is the exception.
    #[serde(default)]
    snapshot: Vec<String>,
}

pub struct Case {
    pub name: String,
    pub program: String,
    pub input: Option<String>,
    pub output: String,
    pub snapshot: Vec<toylang::Backend>,
}

pub fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/corpus")
}

pub fn cases() -> Vec<Case> {
    let dir = dir();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .map(|e| e.expect("readable entry").path())
        .filter(|p| p.extension().is_some_and(|e| e == "yaml"))
        .collect();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let name = path.file_stem().expect("has a stem").to_string_lossy().into_owned();
            let text = std::fs::read_to_string(&path).expect("readable case");
            let fields: Fields = serde_norway::from_str(&text)
                .unwrap_or_else(|e| panic!("{name}: {e}"));
            let snapshot = fields
                .snapshot
                .iter()
                .map(|n| {
                    toylang::Backend::from_name(n)
                        .unwrap_or_else(|| panic!("{name}: `{n}` is not a backend"))
                })
                .collect();
            Case {
                name,
                program: fields.program,
                input: fields.input,
                output: fields.output,
                snapshot,
            }
        })
        .collect()
}
