//! Derives a TextMate grammar for toylang from `parse::Tok`, the lexer's own token vocabulary,
//! instead of a hand-typed grammar file that can silently drift from what `parse::read_tok`
//! actually accepts. `categorize` below matches every `Tok` variant with no wildcard arm, so
//! adding a token to the lexer is a compile error here until it is placed in a highlighting
//! category -- the same "exhaustive on purpose" shape `build.rs`'s `ToRust` impls already use
//! for `tir::Kind`.
//!
//! `tests/syntax_grammar.rs` regenerates this on every `cargo nextest run` and fails if
//! `syntax/toylang.tmLanguage.json` or `editors/vscode/syntaxes/toylang.tmLanguage.json` disagree
//! with it; `cargo run --bin gen_syntax` is the fix when they do.
//!
//! What is NOT derived from `Tok`, because it lives in imperative parsing logic winnow gives no
//! reflection over: string escapes (`parse::read_string`'s match on `esc`) and the number shape
//! (`parse::read_number`/`read_fraction`/`read_exponent`) are hand-transcribed regexes below,
//! each commented with the function to check first if the grammar starts disagreeing with them.

use serde_json::{Value, json};

use crate::parse::Tok;

#[derive(Clone, Copy)]
enum Category {
    /// Declaration-introducing keywords: `fn pub type enum trait impl let`.
    Control,
    /// `and or not` -- boolean connectives spelled as words rather than symbols.
    Logical,
    /// `stdin dsv csv tsv` -- the input-source keywords (`Expr::Stdin`/`Expr::Dsv`).
    Io,
    Operator,
    Punctuation,
}

impl Category {
    fn scope(self) -> &'static str {
        match self {
            Category::Control => "keyword.control.toylang",
            Category::Logical => "keyword.operator.logical.toylang",
            Category::Io => "keyword.other.toylang",
            Category::Operator => "keyword.operator.toylang",
            Category::Punctuation => "punctuation.toylang",
        }
    }
}

/// One instance per fixed-spelling `Tok` variant, in the order `Tok` declares them. `categorize`
/// is what the compiler actually holds to every variant existing; keeping this list in the same
/// order makes a missed entry easy to spot by eye against the enum it mirrors.
const FIXED: &[Tok] = &[
    Tok::Fn,
    Tok::Pub,
    Tok::Type,
    Tok::Enum,
    Tok::Trait,
    Tok::Impl,
    Tok::Let,
    Tok::Stdin,
    Tok::Dsv,
    Tok::Csv,
    Tok::Tsv,
    Tok::And,
    Tok::Or,
    Tok::Not,
    Tok::Plus,
    Tok::Minus,
    Tok::Star,
    Tok::Slash,
    Tok::Percent,
    Tok::Pipe,
    Tok::PipeGt,
    Tok::Comma,
    Tok::Dot,
    Tok::Eq,
    Tok::EqEq,
    Tok::Ne,
    Tok::Bang,
    Tok::Lt,
    Tok::Le,
    Tok::Gt,
    Tok::Ge,
    Tok::LParen,
    Tok::RParen,
    Tok::LBracket,
    Tok::RBracket,
    Tok::LBrace,
    Tok::RBrace,
    Tok::Colon,
    Tok::Semicolon,
    Tok::At,
    Tok::Arrow,
];

/// Exhaustive over every `Tok` variant, no wildcard arm: a new one fails this build until it is
/// placed in a category here, which is also the cue to add it to `FIXED` above.
fn categorize(t: &Tok) -> Category {
    match t {
        Tok::Fn | Tok::Pub | Tok::Type | Tok::Enum | Tok::Trait | Tok::Impl | Tok::Let => {
            Category::Control
        }
        Tok::And | Tok::Or | Tok::Not => Category::Logical,
        Tok::Stdin | Tok::Dsv | Tok::Csv | Tok::Tsv => Category::Io,
        Tok::Plus
        | Tok::Minus
        | Tok::Star
        | Tok::Slash
        | Tok::Percent
        | Tok::Pipe
        | Tok::PipeGt
        | Tok::Dot
        | Tok::Eq
        | Tok::EqEq
        | Tok::Ne
        | Tok::Bang
        | Tok::Lt
        | Tok::Le
        | Tok::Gt
        | Tok::Ge
        | Tok::Arrow => Category::Operator,
        Tok::Comma
        | Tok::LParen
        | Tok::RParen
        | Tok::LBracket
        | Tok::RBracket
        | Tok::LBrace
        | Tok::RBrace
        | Tok::Colon
        | Tok::Semicolon
        | Tok::At => Category::Punctuation,
        Tok::Str(_) | Tok::Int(_) | Tok::Float(_) | Tok::Ident(_) | Tok::Eof => {
            unreachable!("not a fixed-spelling token; never constructed into FIXED")
        }
    }
}

/// The literal text a fixed token spells out, read back off `Tok`'s own `Display` impl -- the
/// same string `parse.rs`'s error messages assert on in 105+ snapshot tests -- rather than
/// retyped here. Every `Display` arm reachable from a `FIXED` entry renders as `` `text` ``.
fn spelling(t: &Tok) -> String {
    t.to_string()
        .trim_start_matches('`')
        .trim_end_matches('`')
        .to_string()
}

fn regex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if "\\^$.|?*+()[]{}".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// One `patterns` entry matching any spelling in `words` under `scope`. Longest spelling first,
/// so e.g. `|>` is tried before `|` and `==` before `=` -- with every operator sharing one scope
/// this only affects match boundaries, never the rendered color, but it is the honest regex
/// either way. `\b`-wrapped only for word-shaped spellings (letters), since it is meaningless
/// (never matches) around a purely symbolic one.
fn group(scope: &str, words: &[String], word_shaped: bool) -> Value {
    let mut sorted: Vec<&String> = words.iter().collect();
    sorted.sort_by_key(|w| std::cmp::Reverse(w.len()));
    let alts: Vec<String> = sorted.iter().map(|w| regex_escape(w)).collect();
    let body = alts.join("|");
    let pattern = if word_shaped {
        format!(r"\b(?:{body})\b")
    } else {
        format!("(?:{body})")
    };
    json!({ "name": scope, "match": pattern })
}

/// The toylang TextMate grammar, built from `FIXED`/`categorize` plus the hand-transcribed
/// literal and comment patterns documented above. Returned as a `Value` so both `gen_syntax`
/// (writes it to disk) and `tests/syntax_grammar.rs` (checks the disk copies against it) share
/// one generation path.
pub fn grammar() -> Value {
    let mut by_category: Vec<(&'static str, Vec<String>)> = Vec::new();
    for tok in FIXED {
        let scope = categorize(tok).scope();
        let text = spelling(tok);
        match by_category.iter_mut().find(|(s, _)| *s == scope) {
            Some((_, words)) => words.push(text),
            None => by_category.push((scope, vec![text])),
        }
    }

    let word_shaped = |scope: &str| {
        scope.starts_with("keyword.control")
            || scope == "keyword.operator.logical.toylang"
            || scope == "keyword.other.toylang"
    };

    let mut patterns = vec![
        // `#` runs to end of line (`skip_trivia`); no block-comment form exists.
        json!({ "name": "comment.line.number-sign.toylang", "match": "#.*$" }),
        // Escapes are `read_string`'s match on `esc`: `" \ / n t r`, JSON's set minus `\u`.
        json!({
            "name": "string.quoted.double.toylang",
            "begin": "\"",
            "end": "\"",
            "patterns": [
                { "name": "constant.character.escape.toylang", "match": "\\\\[\"\\\\/ntr]" }
            ]
        }),
        // `read_number`: digits, then an optional `.digits` fraction and an optional
        // `e`/`E`-exponent with an optional sign -- either makes it a Float (ADR 0007).
        json!({
            "name": "constant.numeric.toylang",
            "match": r"\b[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?\b"
        }),
    ];
    for (scope, words) in &by_category {
        patterns.push(group(scope, words, word_shaped(scope)));
    }

    json!({
        "$schema": "https://raw.githubusercontent.com/martinring/tmlanguage/master/tmlanguage.json",
        "name": "toylang",
        "scopeName": "source.toylang",
        "fileTypes": ["toy"],
        "patterns": patterns
    })
}
