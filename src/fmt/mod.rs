//! The formatter: re-renders a parsed file as toylang source, in one canonical style. Unlike
//! the `emit_*` backends this walks `ast::File`, not `tir::Program` -- a formatter has to work
//! on a program that does not type-check, and it must not print the prelude that
//! `prelude::inject` would otherwise splice in.
//!
//! One rendering per AST (maintainer ruling, 2026-09-19): the output is a function of the parsed
//! tree and nothing else. The author's line breaks, blank-line grouping, and redundant parens
//! are not in the tree, so they do not survive. Two templates render that one tree:
//!
//! - `multi_line`, the file template: a 69-column width that is the template's own property,
//!   not a rule a program is held to (a node with no seam to break at overflows rather than
//!   failing), and comments placed by `comments`.
//! - `one_line`, the one-line template: the whole program on a single line with no width and
//!   no comments. It is also every node's single-line rendering, which the file template tries
//!   first and breaks only when it does not fit.
//!
//! `parens` holds the one decision both share: which parens the tree needs, since the AST does
//! not record which ones the source had.
//!
//! The maintainer's own hand-formatted sample, examples/shapes.toy (2026-09-19), is the ground
//! truth for the file template's layout: 2-space indent, 69 columns, padded braces everywhere
//! (`Circle { r }`, `{ r: 3 }`), a broken pipeline's `|` at its subject's column, a broken
//! chain's arms aligned with the first arm's text. Everything else is fit-based: whatever fits
//! on one line stays there.
//!
//! A call's own parens are optional wherever the grammar can read the argument back
//! unambiguously (maintainer ruling, 2026-09-19: force bare application except where parens
//! are really needed), so `f(x)` prints `f x` and `f({a: 1})` prints `f { a: 1 }`. `bare_arg_ok`
//! in `one_line.rs` decides which arguments qualify, mirroring `parse.rs::ident_expr` exactly;
//! its own doc comment has the grammar's reasoning.

mod comments;
mod multi_line;
mod one_line;
mod parens;

pub use multi_line::{emit, emit_module};
pub use one_line::{emit_module_one_line, emit_one_line};

use crate::ast::{Alias, Def, EnumDecl, Expr, File, ImplDecl, Span, TraitDecl};

pub(super) enum Item<'a> {
    Alias(&'a Alias),
    Enum(&'a EnumDecl),
    Trait(&'a TraitDecl),
    Impl(&'a ImplDecl),
    Def(&'a Def),
}

impl Item<'_> {
    fn span(&self) -> Span {
        match self {
            Item::Alias(a) => a.span,
            Item::Enum(e) => e.span,
            Item::Trait(t) => t.span,
            Item::Impl(i) => i.span,
            Item::Def(d) => d.span,
        }
    }
}

/// Every declaration, back in the order it was written.
///
/// The AST groups declarations by kind, losing their interleaving in the source; sorting by span
/// start puts them back, which is what makes the output idempotent -- reformatting an
/// already-sorted file is a no-op re-sort.
pub(super) fn decls_in_source_order<'a>(
    aliases: &'a [Alias],
    enums: &'a [EnumDecl],
    traits: &'a [TraitDecl],
    impls: &'a [ImplDecl],
    defs: &'a [Def],
) -> Vec<Item<'a>> {
    let mut items: Vec<Item<'a>> = Vec::new();
    items.extend(aliases.iter().map(Item::Alias));
    items.extend(enums.iter().map(Item::Enum));
    items.extend(traits.iter().map(Item::Trait));
    items.extend(impls.iter().map(Item::Impl));
    items.extend(defs.iter().map(Item::Def));
    items.sort_by_key(|item| item.span().start);
    items
}

/// Parses `src` and formats it.
///
/// Two file shapes reach a formatter that walks a project: a program, and a module --
/// declarations with no trailing expression, which `parse` rejects outright and which
/// `prelude.toy` is the one instance of here. Which one a file is only shows up in the parsing,
/// so both are tried. When neither parse succeeds, the error reported is whichever got further
/// into the file, so a typo halfway down a module is not reported as a missing program body.
pub fn format_source(src: &str) -> Result<String, crate::error::Error> {
    let as_program = match crate::parse::parse(src) {
        Ok(file) => return Ok(emit(&file)),
        Err(e) => e,
    };
    match crate::parse::parse_module(src) {
        Ok(module) => Ok(emit_module(&module)),
        Err(as_module) if as_module.span.start > as_program.span.start => Err(as_module),
        Err(_) => Err(as_program),
    }
}

/// The one-line template, as a source-to-source function.
///
/// Refuses two shapes as having no one-line form (maintainer ruling, 2026-09-19). A program
/// with a `let` block, since the grammar reads a block one binding per line. And a program
/// whose one-line form would read differently: the grammar separates the last definition's
/// body from the program body by a line break alone, and bare application makes a name
/// followed by an atom a call, so `fn g(x: Int) -> Int = x` then `g(1)` becomes `x g(1)`, the
/// call `x(g(1))`. Rather than enumerate the shapes, the rendering is parsed back and compared,
/// tree to tree, through the file template; a mismatch names the definition whose body was
/// swallowed.
pub fn format_one_line(src: &str) -> Result<String, crate::error::Error> {
    let as_program = match crate::parse::parse(src) {
        Ok(mut file) => {
            if let Some(d) = file
                .defs
                .iter()
                .find(|d| matches!(d.body, Expr::Let { .. }))
            {
                return Err(crate::error::Error::new(
                    d.span,
                    "this program has no one-line form: a `let` block is one binding per line",
                ));
            }
            let line = emit_one_line(&file);
            let reread = crate::parse::parse(&line).map_err(|e| one_line_error(&file, e.msg))?;
            // The one-line form carries no comments, so the trees are compared without them.
            file.comments.clear();
            if emit(&reread) != emit(&file) {
                return Err(one_line_error(&file, "it reads as a different program"));
            }
            return Ok(line);
        }
        Err(e) => e,
    };
    match crate::parse::parse_module(src) {
        // A module is declarations only, each starting with a keyword no argument can be, so
        // its one-line form always reads back the same.
        Ok(module) => Ok(emit_module_one_line(&module)),
        Err(as_module) if as_module.span.start > as_program.span.start => Err(as_module),
        Err(_) => Err(as_program),
    }
}

/// The one-line failure, placed at the last definition: the seam between its body and the
/// program body is where a one-line rendering loses its separator.
fn one_line_error(file: &File, why: impl std::fmt::Display) -> crate::error::Error {
    let span = file
        .defs
        .iter()
        .map(|d| d.span)
        .max_by_key(|s| s.start)
        .unwrap_or_else(|| file.body.span());
    crate::error::Error::new(
        span,
        format!(
            "this program has no one-line form: {why} (a body followed by an atom on the same \
             line reads as a call, and the grammar has no other separator)"
        ),
    )
}
