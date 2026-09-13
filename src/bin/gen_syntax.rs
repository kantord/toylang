//! Regenerates the toylang syntax-highlighting grammar from the lexer's own token vocabulary.
//! See `syntax_gen` for how the grammar itself is derived; this binary only decides where the
//! result lands. Run after any change to `parse::Tok`'s keyword or operator set --
//! `tests/syntax_grammar.rs` fails `cargo nextest run` until you do.

use std::path::Path;

use anyhow::Result;

fn main() -> Result<()> {
    let pretty = serde_json::to_string_pretty(&toylang::syntax_gen::grammar())?;
    let contents = format!("{pretty}\n");

    // Two copies of the same generated content: the site imports the first directly, and the
    // second is what makes editors/vscode/ a self-contained extension folder a `vsce package`
    // can zip up without reaching outside itself.
    for path in [
        "syntax/toylang.tmLanguage.json",
        "editors/vscode/syntaxes/toylang.tmLanguage.json",
    ] {
        let path = Path::new(path);
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(path, &contents)?;
        println!("wrote {}", path.display());
    }
    Ok(())
}
