//! The compiler's own config file, `toylang.conf.yaml`.
//!
//! Hosts the escape hatch the Web target needs: browser-side implementations for the
//! runtime helpers that would otherwise read stdin through node's `fs`. A program that needs
//! one and finds no replacement is refused at compile time for the Web target, rather than
//! emitting code that would silently break in a browser. Node never consults any of this.

use std::path::PathBuf;

use serde::Deserialize;

/// What a Web-target compile may supply in place of a node-only stdin reader. The text
/// is emitted verbatim, so it has to define the function name the emitted code calls.
#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Web {
    /// A `tl_read_input()` returning the raw stdin text an `input`/`inputs` program splits
    /// and parses. Used by both: the two share the read, and differ only in what follows it.
    #[serde(default)]
    pub input: Option<String>,
    /// A `tl_collect_lines()` returning the raw lines a `lines`/`dsv` program iterates over.
    #[serde(default)]
    pub lines: Option<String>,
    /// A `tl_read_line()` returning the next raw line, or `null` at end-of-input, for a
    /// stream-typed stdin pipeline (the fused loop every backend compiles one into).
    #[serde(default)]
    pub read_line: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub web: Web,
}

impl Config {
    /// The config the current directory's chain names, or an empty one when no
    /// `toylang.conf.yaml` exists anywhere up the tree. A malformed file is an error: a
    /// config whose typo was silently ignored would be a case nobody wrote down.
    pub fn load() -> Result<Config, String> {
        match find() {
            None => Ok(Config::default()),
            Some(path) => {
                let text = std::fs::read_to_string(&path).map_err(|e| {
                    format!(
                        "{}: could not read the compiler config: {e}",
                        path.display()
                    )
                })?;
                serde_norway::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
            }
        }
    }
}

/// Walk upward from the current directory until a `toylang.conf.yaml` is found.
fn find() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join("toylang.conf.yaml");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_web_config_parses() {
        let cfg: Config = serde_norway::from_str(
            "web:\n  input: |\n    function tl_read_input() { return \"\"; }\n",
        )
        .expect("parses");
        assert!(cfg.web.input.is_some());
    }
}
