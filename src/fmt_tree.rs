//! The project-wide half of `toylang fmt`: walk a folder, and either report which files are not
//! in canonical form or rewrite them. The filter half is one `toylang::fmt` call, and stays in
//! `main.rs`.
//!
//! Nothing here stops at the first problem. A file the formatter cannot handle is recorded and
//! the walk continues, so one unparseable file does not hide the state of the rest of the tree.

use std::path::{Path, PathBuf};

/// Only `.toy` files. A `toylang` fence inside a markdown page is formattable source too, and
/// `tests/fmt_examples.rs` holds every fence under `docs/` to canonical form, but rewriting one
/// means editing the page around it: not something a walk should do to a reader's prose.
const EXTENSION: &str = "toy";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Report what would change, write nothing.
    Check,
    /// Rewrite in place.
    Write,
}

#[derive(Default)]
pub struct Report {
    /// The files not in canonical form: listed under `Check`, rewritten under `Write`.
    pub changed: Vec<PathBuf>,
    /// What the walk could not format, and why: an unreadable file or folder, a source the
    /// parser rejected, a rewrite that failed.
    pub failed: Vec<(PathBuf, String)>,
}

impl Report {
    /// The exit condition both modes share: a run that found something is a failed run, whether
    /// or not it did anything about it.
    pub fn is_clean(&self) -> bool {
        self.changed.is_empty() && self.failed.is_empty()
    }
}

/// Checks, or formats, every `.toy` file under `root`.
pub fn run(root: &Path, mode: Mode) -> Report {
    let mut report = Report::default();
    let mut paths = Vec::new();
    collect(root, &mut paths, &mut report);
    // Directory order is whatever the filesystem hands back, which differs between machines and
    // between runs on the same one. The listing is the output of this command, so it is sorted.
    paths.sort();
    for path in paths {
        format_one(&path, mode, &mut report);
    }
    report
}

fn format_one(path: &Path, mode: Mode, report: &mut Report) {
    let mut fail = |e: String| report.failed.push((path.to_path_buf(), e));
    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => return fail(e.to_string()),
    };
    let formatted = match crate::fmt(&src) {
        Ok(formatted) => formatted,
        Err(e) => return fail(e.to_string()),
    };
    if formatted == src {
        return;
    }
    if mode == Mode::Write
        && let Err(e) = std::fs::write(path, &formatted)
    {
        return fail(e.to_string());
    }
    report.changed.push(path.to_path_buf());
}

/// Hidden entries are skipped: a walk from the current folder would otherwise descend into
/// `.git`, and a leading dot is the usual way a folder says it is not the project's own source.
/// Symlinks are skipped as well, both kinds: a link cycle would hang the walk, and rewriting
/// through a link edits a file outside the tree that was walked.
fn collect(dir: &Path, out: &mut Vec<PathBuf>, report: &mut Report) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(e) => {
            report.failed.push((dir.to_path_buf(), e.to_string()));
            return;
        }
    };
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(e) => {
                report.failed.push((dir.to_path_buf(), e.to_string()));
                continue;
            }
        };
        if entry.file_name().to_string_lossy().starts_with('.') {
            continue;
        }
        let path = entry.path();
        match entry.file_type() {
            Ok(t) if t.is_dir() => collect(&path, out, report),
            Ok(t) if t.is_file() && path.extension().is_some_and(|e| e == EXTENSION) => {
                out.push(path);
            }
            Ok(_) => {}
            Err(e) => report.failed.push((path, e.to_string())),
        }
    }
}
