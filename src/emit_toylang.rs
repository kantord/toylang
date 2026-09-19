//! The formatter's own backend: re-renders a parsed file as toylang source, in one canonical
//! style. Unlike the other `emit_*` backends this walks `ast::File`, not `tir::Program` -- a
//! formatter has to work on a program that does not type-check, and it must not print the
//! prelude that `prelude::inject` would otherwise splice in.
//!
//! Two things drive the design. First, minimal-but-correct parenthesization: the AST does not
//! record which parens the source actually used, so every paren in the output is one this module
//! decided it needed, by comparing a node's own operator power against the power the position it
//! sits in requires -- the same table `parse.rs::infix_power` parses with, walked in reverse.
//! Second, the maintainer's own sample (docs/examples/euler/01-multiples-of-3-and-5.md) is the
//! only ground truth for layout: a function whose signature-plus-body fits on one line stays on
//! one line; otherwise the body moves to its own indented line, and a binary chain that still
//! does not fit breaks at its outermost operator, trailing the operator on the first line.
//! Match-arm chains extend that same "trailing operator, one extra indent" rule by analogy,
//! since nothing in the sample pins their layout directly. A pipeline that does not fit is the
//! one exception: it breaks one stage per line, with `|` leading each continuation line so the
//! pipes form a vertical column (maintainer directive, issue #101), rather than trailing like
//! the other chains.
//!
//! Two style choices worth naming since nothing in the grammar forces them: calls are always
//! written with explicit parens (`f(x)`, never the bare `f x` or brace-shorthand `f{...}`),
//! matching both calls in the sample; and the wrap width is 80 columns, backed out from the
//! sample itself -- the one line it left alone is 44 columns, and the two it broke are 89 and
//! 118.
//!
//! One rendering per AST (maintainer ruling, 2026-09-19): the output is a function of the parsed
//! tree and nothing else. The author's line breaks, blank-line grouping, and redundant parens
//! are not in the tree, so they do not survive. The 80-column width is a property of this
//! module's file template, not a rule a program is held to: a node with no seam to break at
//! overflows rather than failing. A second template, the one-line form, renders the same tree
//! on a single line with no width at all; for expressions it is the `print_expr_compact` path
//! the file template itself tries first.
//!
//! Comments are the one input that lives beside the tree rather than in it. `parse` records
//! every `#` line with its span (`ast::Comment`), and `Comments` below hands them out in source
//! order as the printer walks the line-owning items: declarations, `let` bindings, a `let`
//! block's value, the program body. A comment on its own line goes above the next such item; a
//! comment trailing code stays at the end of the line that item lands on; an own-line comment
//! inside an expression rises to the top of the item holding it, since a re-rendered expression
//! has no line for it to stay on. The one piece of author spacing kept is the blank line after a
//! comment, which is what tells a file banner from a doc comment.

use crate::ast::{
    Alias, BinOp, Comment, Def, EnumDecl, Expr, FieldsPattern, File, ImplDecl, ImplMethod, LogicOp,
    MatchArm, Module, Param, ParamShape, Pattern, Span, TraitDecl, TraitMethodSig, TypeExpr,
    Variant,
};

const WIDTH: usize = 80;
const INDENT: usize = 4;

// Mirrors parse.rs's precedence table exactly (`infix_power`, `PIPE_LEFT`/`PIPE_RIGHT`,
// `COND_POWER`, `OR_LEFT`/`OR_RIGHT`, `NOT_POWER`): the numbers a paren decision here has to
// answer to are the parser's, not this module's own invention.
const PIPE_LEFT: u8 = 1;
const PIPE_RIGHT: u8 = 2;
const COND_POWER: u8 = 3;
const NOT_POWER: u8 = 8;

/// A match arm's body is printed at a power just above `or`'s, which is how the parser's second
/// reading of `or` -- the arm separator, which has no power at all -- reaches this table: a bare
/// disjunction in an arm body is the one expression whose parens the powers alone would not ask
/// for, and the one the parser would otherwise read as two arms.
const ARM_BODY: u8 = 5;

fn bin_power(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => (8, 9),
        BinOp::Add | BinOp::Sub => (10, 11),
        BinOp::Mul | BinOp::Div | BinOp::Rem => (12, 13),
    }
}

fn logic_power(op: LogicOp) -> (u8, u8) {
    match op {
        LogicOp::Or => (4, 5),
        LogicOp::And => (6, 7),
    }
}

/// Where a child expression sits, in terms of what the parser would have accepted bare at that
/// spot -- everything `needs_parens` and the wrapping printer need to know to reproduce the tree
/// exactly, and nothing else.
#[derive(Clone, Copy)]
enum Ctx {
    /// A fresh `self.expr(m)` call: File/Def body, a call argument (always real parens here, so
    /// always reset to `Expr(0)` inside them), `Index`'s bracketed expression, a match arm's
    /// body. The one context where a bare `Pipe` and, when `m` is loose enough, a bare `Match`
    /// are both reachable.
    Expr(u8),
    /// A `self.operand(m)` call: a `Binary` child, or `Pattern::Guard`'s expression. `Pipe` and
    /// `Match` are never reachable bare here -- only `expr()` parses those.
    Operand(u8),
    /// The base of a postfix chain (`Field`/`Index`/`Project`/`Unwrap`), or a `Call`/`Variant`'s
    /// argument print used only when NOT already inside real parens. Nothing compound is
    /// reachable bare here -- not even `Neg`.
    Atom,
    /// `Neg`'s base: like `Atom`, except a nested `Neg` is reachable bare (`- -a`).
    Unary,
}

/// The left binding power of whatever operator `e` is, for the three contexts that decide by
/// power alone. `None` for everything else, which each context rules on by kind.
fn left_power(e: &Expr) -> Option<u8> {
    match e {
        Expr::Binary { op, .. } => Some(bin_power(*op).0),
        Expr::Logic { op, .. } => Some(logic_power(*op).0),
        Expr::Not { .. } => Some(NOT_POWER),
        _ => None,
    }
}

fn needs_parens(e: &Expr, ctx: Ctx) -> bool {
    // A `let` block is only ever a function body, never a sub-expression, so no child position
    // can hold one; treating it as needing parens everywhere is the safe dead arm.
    if matches!(e, Expr::Let { .. }) {
        return true;
    }
    match ctx {
        Ctx::Atom => matches!(
            e,
            Expr::Binary { .. }
                | Expr::Logic { .. }
                | Expr::Not { .. }
                | Expr::Pipe { .. }
                | Expr::Match { .. }
                | Expr::Neg { .. }
        ),
        Ctx::Unary => {
            matches!(
                e,
                Expr::Binary { .. }
                    | Expr::Logic { .. }
                    | Expr::Not { .. }
                    | Expr::Pipe { .. }
                    | Expr::Match { .. }
            )
        }
        Ctx::Operand(m) => match e {
            Expr::Pipe { .. } | Expr::Match { .. } => true,
            _ => left_power(e).is_some_and(|p| p < m),
        },
        Ctx::Expr(m) => match e {
            Expr::Pipe { .. } => PIPE_LEFT < m,
            Expr::Match { .. } => m > PIPE_RIGHT,
            _ => left_power(e).is_some_and(|p| p < m),
        },
    }
}

fn fits(s: &str, indent: usize) -> bool {
    indent + s.chars().count() <= WIDTH
}

fn pad(n: usize) -> String {
    " ".repeat(n)
}

pub fn emit(file: &File) -> String {
    let mut comments = Comments::new(&file.comments);
    let mut out = String::new();
    for item in decls_in_source_order(
        &file.aliases,
        &file.enums,
        &file.traits,
        &file.impls,
        &file.defs,
    ) {
        out.push_str(&print_decl(&item, &mut comments));
        out.push_str("\n\n");
    }
    let leading = comments.take_before(file.body.span().end);
    out.push_str(&comment_lines(&leading, 0));
    let body = print_expr_wrapped(&file.body, Ctx::Expr(0), 0);
    out.push_str(&with_trailing(body, comments.take_trailing()));
    out.push('\n');
    out.push_str(&comment_lines(&comments.take_rest(), 0));
    ensure_single_newline(out)
}

/// A module is the declarations alone: no trailing expression to separate them from, so they end
/// the file rather than each being followed by a blank line the way `emit` writes them.
pub fn emit_module(module: &Module) -> String {
    let mut comments = Comments::new(&module.comments);
    let decls: Vec<String> = decls_in_source_order(
        &module.aliases,
        &module.enums,
        &module.traits,
        &module.impls,
        &module.defs,
    )
    .iter()
    .map(|item| print_decl(item, &mut comments))
    .collect();
    let mut out = decls.join("\n\n");
    out.push('\n');
    out.push_str(&comment_lines(&comments.take_rest(), 0));
    if out.trim().is_empty() {
        return String::new();
    }
    ensure_single_newline(out)
}

/// The one-line template: the same tree on a single line, with no width. Declarations and the
/// body are separated by one space, a `let` block is `let a = .. let b = .. value`, and
/// trait and impl methods sit inside their braces separated by spaces. Comments are dropped:
/// a `#` runs to the end of the line, so none can sit inside one.
///
/// The grammar reads a line break as the separator between a `let` value and the block's
/// value, and between the last definition's body and the program body; on one line, a body
/// ending in a name followed by an atom re-reads as a bare application (`x g(1)` is `x(g(1))`).
/// This renderer does not guard against that -- `fmt_one_line` in lib.rs re-parses its output
/// and refuses the program when the reading changed.
pub fn emit_one_line(file: &File) -> String {
    let mut parts: Vec<String> = decls_in_source_order(
        &file.aliases,
        &file.enums,
        &file.traits,
        &file.impls,
        &file.defs,
    )
    .iter()
    .map(print_decl_one_line)
    .collect();
    parts.push(print_expr_compact(&file.body, Ctx::Expr(0)));
    format!("{}\n", parts.join(" "))
}

pub fn emit_module_one_line(module: &Module) -> String {
    let decls: Vec<String> = decls_in_source_order(
        &module.aliases,
        &module.enums,
        &module.traits,
        &module.impls,
        &module.defs,
    )
    .iter()
    .map(print_decl_one_line)
    .collect();
    if decls.is_empty() {
        return String::new();
    }
    format!("{}\n", decls.join(" "))
}

fn print_decl_one_line(item: &Item) -> String {
    match item {
        Item::Alias(a) => print_alias(a),
        Item::Enum(e) => print_enum_compact(e),
        Item::Trait(t) => {
            let methods: Vec<String> = t.methods.iter().map(print_trait_method).collect();
            format!("{} {}", trait_head(t), brace_one_line(&methods))
        }
        Item::Impl(i) => {
            let methods: Vec<String> = i.methods.iter().map(print_impl_method_one_line).collect();
            format!("{} {}", impl_head(i), brace_one_line(&methods))
        }
        Item::Def(d) => print_def_one_line(d),
    }
}

fn brace_one_line(items: &[String]) -> String {
    if items.is_empty() {
        "{}".to_string()
    } else {
        format!("{{ {} }}", items.join(" "))
    }
}

fn print_def_one_line(d: &Def) -> String {
    let pub_prefix = if d.is_pub { "pub " } else { "" };
    if d.hoisted {
        return format!(
            "{pub_prefix}fn {} = {}",
            d.name,
            print_expr_compact(&d.body, Ctx::Expr(0))
        );
    }
    let ret = d
        .ret
        .as_ref()
        .map(print_type)
        .expect("a non-hoisted definition always writes a return type");
    format!(
        "{pub_prefix}fn {}({}) -> {ret} = {}",
        d.name,
        print_param(&d.param),
        print_body_one_line(&d.body)
    )
}

fn print_impl_method_one_line(m: &ImplMethod) -> String {
    format!(
        "fn {}({}) -> {} = {}",
        m.name,
        print_param(&m.param),
        print_type(&m.ret),
        print_body_one_line(&m.body)
    )
}

/// A definition body: a `let` block is its bindings and value in a row, since `let` is a
/// keyword and cannot be read as an argument; anything else is its compact form.
fn print_body_one_line(body: &Expr) -> String {
    let Expr::Let { bindings, body, .. } = body else {
        return print_expr_compact(body, Ctx::Expr(0));
    };
    let mut parts: Vec<String> = bindings
        .iter()
        .map(|(n, v)| format!("let {n} = {}", print_expr_compact(v, Ctx::Expr(0))))
        .collect();
    parts.push(print_expr_compact(body, Ctx::Expr(0)));
    parts.join(" ")
}

/// A comment's `blank_after` can leave a blank line at the very end; a file ends in exactly one
/// newline.
fn ensure_single_newline(out: String) -> String {
    format!("{}\n", out.trim_end())
}

/// The file's comments, handed out in source order as the printer walks the line-owning items.
/// The placement rules are in the module doc.
struct Comments<'a> {
    list: &'a [Comment],
    next: usize,
}

impl<'a> Comments<'a> {
    fn new(list: &'a [Comment]) -> Self {
        Comments { list, next: 0 }
    }

    /// Every comment not yet taken that starts before `limit`.
    fn take_before(&mut self, limit: usize) -> Vec<&'a Comment> {
        let mut out = Vec::new();
        while let Some(c) = self.list.get(self.next)
            && c.span.start < limit
        {
            out.push(c);
            self.next += 1;
        }
        out
    }

    /// The comment on the line the last printed item ended on, if there is one.
    fn take_trailing(&mut self) -> Option<&'a Comment> {
        let c = self.list.get(self.next)?;
        if c.own_line {
            return None;
        }
        self.next += 1;
        Some(c)
    }

    fn take_rest(&mut self) -> Vec<&'a Comment> {
        self.take_before(usize::MAX)
    }
}

/// Own-line comments, one per line at `indent`, keeping the blank line after any that had one.
fn comment_lines(comments: &[&Comment], indent: usize) -> String {
    let mut out = String::new();
    for c in comments {
        out.push_str(&pad(indent));
        out.push('#');
        out.push_str(&c.text);
        out.push('\n');
        if c.blank_after {
            out.push('\n');
        }
    }
    out
}

/// `rendered` with `comment`, if any, at the end of its last line.
fn with_trailing(mut rendered: String, comment: Option<&Comment>) -> String {
    if let Some(c) = comment {
        rendered.push_str(" #");
        rendered.push_str(&c.text);
    }
    rendered
}

enum Item<'a> {
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
fn decls_in_source_order<'a>(
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

/// One declaration with its comments: the own-line ones before it (and, for everything but a
/// `let`-bodied definition, inside it) above, and the one trailing its last line at its end.
fn print_decl(item: &Item, comments: &mut Comments) -> String {
    let span = item.span();
    let mut leading = comments.take_before(span.start);
    let rendered = match item {
        Item::Def(d) if matches!(d.body, Expr::Let { .. }) => print_let_def(d, comments),
        _ => {
            leading.extend(comments.take_before(span.end));
            match item {
                Item::Alias(a) => print_alias(a),
                Item::Enum(e) => print_enum(e),
                Item::Trait(t) => print_trait(t),
                Item::Impl(i) => print_impl(i),
                Item::Def(d) => print_def(d),
            }
        }
    };
    let mut out = comment_lines(&leading, 0);
    out.push_str(&with_trailing(rendered, comments.take_trailing()));
    out
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

fn print_alias(a: &Alias) -> String {
    format!("type {} = {}", a.name, print_type(&a.ty))
}

fn print_param(p: &Option<Param>) -> String {
    match p {
        None => String::new(),
        Some(p) => match &p.shape {
            ParamShape::Name(name, _) => format!("{name}: {}", print_type(&p.ty)),
            ParamShape::Fields(f) => {
                format!("{{{}}}: {}", print_fields_pattern(f), print_type(&p.ty))
            }
        },
    }
}

/// `fn name(param) -> ret`, on one line when it and the ` =` after it fit at `indent`. Otherwise
/// the parameter goes on its own line one level in, and a record parameter type that still does
/// not fit there breaks one field per line. A nullary signature has nothing to break at.
fn print_sig(
    pub_prefix: &str,
    name: &str,
    param: &Option<Param>,
    ret: &str,
    indent: usize,
) -> String {
    let one_line = format!("{pub_prefix}fn {name}({}) -> {ret}", print_param(param));
    if fits(&format!("{one_line} ="), indent) {
        return one_line;
    }
    let Some(p) = param else {
        return one_line;
    };
    let inner = indent + INDENT;
    let compact = print_param(param);
    let param_str = if fits(&compact, inner) {
        compact
    } else {
        print_param_wrapped(p, inner)
    };
    format!(
        "{pub_prefix}fn {name}(\n{}{param_str}\n{}) -> {ret}",
        pad(inner),
        pad(indent)
    )
}

fn print_param_wrapped(p: &Param, indent: usize) -> String {
    let shape = match &p.shape {
        ParamShape::Name(name, _) => name.clone(),
        ParamShape::Fields(f) => format!("{{{}}}", print_fields_pattern(f)),
    };
    format!("{shape}: {}", print_type_wrapped(&p.ty, indent))
}

/// A record type one field per line; every other type has no seam and prints compact.
fn print_type_wrapped(t: &TypeExpr, indent: usize) -> String {
    match t {
        TypeExpr::Record { fields, .. } => {
            let rendered: Vec<String> = fields
                .iter()
                .map(|(n, t)| format!("{n}: {}", print_type(t)))
                .collect();
            wrap_delim("{", &rendered, "}", indent)
        }
        _ => print_type(t),
    }
}

/// `sig = body` on one line when that fits at `indent`; otherwise the body on its own line one
/// level in. A signature that already broke never takes its body on the closing line.
fn print_signed(sig: String, body: &Expr, indent: usize) -> String {
    if !sig.contains('\n') {
        let one_line = format!("{sig} = {}", print_expr_compact(body, Ctx::Expr(0)));
        if fits(&one_line, indent) {
            return one_line;
        }
    }
    let inner = indent + INDENT;
    format!(
        "{sig} =\n{}{}",
        pad(inner),
        print_expr_wrapped(body, Ctx::Expr(0), inner)
    )
}

/// A definition whose body is a `let` block: the signature, then one `let` line per binding,
/// then the value, each indented one level. The block has no one-line form in this template.
/// Each binding and the value is a line-owning item, so each takes its own comments: the
/// own-line ones before it above it, the one trailing it at its end.
fn print_let_def(d: &Def, comments: &mut Comments) -> String {
    let Expr::Let { bindings, body, .. } = &d.body else {
        unreachable!("print_let_def is only called on a `let` body")
    };
    let pub_prefix = if d.is_pub { "pub " } else { "" };
    let ret = d
        .ret
        .as_ref()
        .map(print_type)
        .expect("a `let` block is only ever the body of a signed definition");
    let mut out = format!("{} =\n", print_sig(pub_prefix, &d.name, &d.param, &ret, 0));
    // The parser holds a binding to one line (`parse.rs::def_body`), so a value has no wrapped
    // form: an overlong one is overlong.
    for (n, v) in bindings {
        let leading = comments.take_before(v.span().start);
        out.push_str(&comment_lines(&leading, INDENT));
        let line = format!("let {n} = {}", print_expr_compact(v, Ctx::Expr(0)));
        out.push_str(&pad(INDENT));
        out.push_str(&with_trailing(line, comments.take_trailing()));
        out.push('\n');
    }
    let leading = comments.take_before(body.span().end);
    out.push_str(&comment_lines(&leading, INDENT));
    out.push_str(&pad(INDENT));
    out.push_str(&print_expr_wrapped(body, Ctx::Expr(0), INDENT));
    out
}

fn print_def(d: &Def) -> String {
    let pub_prefix = if d.is_pub { "pub " } else { "" };
    // A hoisted definition (`fn name = Msg(...)`, gh:152) writes no signature: parameter and
    // return are both inferred from the body, so neither has a spelling to print.
    if d.hoisted {
        let one_line = format!(
            "{pub_prefix}fn {} = {}",
            d.name,
            print_expr_compact(&d.body, Ctx::Expr(0))
        );
        if fits(&one_line, 0) {
            return one_line;
        }
        let body = print_expr_wrapped(&d.body, Ctx::Expr(0), INDENT);
        return format!("{pub_prefix}fn {} =\n{}{body}", d.name, pad(INDENT));
    }
    let ret = match &d.ret {
        Some(ret) => print_type(ret),
        // A definition that is not hoisted always writes its return type, so a `None` here is
        // a parser/checker invariant, never a legal program.
        None => unreachable!("a non-hoisted definition always writes a return type"),
    };
    print_signed(
        print_sig(pub_prefix, &d.name, &d.param, &ret, 0),
        &d.body,
        0,
    )
}

fn print_variant_decl(v: &Variant) -> String {
    match &v.payload {
        None => v.name.clone(),
        Some(TypeExpr::Record { fields, .. }) => {
            let fields_str = fields
                .iter()
                .map(|(n, t)| format!("{n}: {}", print_type(t)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{}{{{fields_str}}}", v.name)
        }
        Some(t) => format!("{}({})", v.name, print_type(t)),
    }
}

/// `<T, ...>`, shared by an enum's and an impl's own declared type parameters: empty when there
/// are none.
fn print_type_params(params: &[(String, crate::ast::Span)]) -> String {
    if params.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = params.iter().map(|(n, _)| n.as_str()).collect();
        format!("<{}>", names.join(", "))
    }
}

fn enum_head(e: &EnumDecl) -> String {
    let pub_prefix = if e.is_pub { "pub " } else { "" };
    format!(
        "{pub_prefix}enum {}{}",
        e.name,
        print_type_params(&e.params)
    )
}

fn print_enum_compact(e: &EnumDecl) -> String {
    let variants: Vec<String> = e.variants.iter().map(print_variant_decl).collect();
    if variants.is_empty() {
        return format!("{} {{}}", enum_head(e));
    }
    format!("{} {{ {} }}", enum_head(e), variants.join(", "))
}

fn print_enum(e: &EnumDecl) -> String {
    let compact = print_enum_compact(e);
    if e.variants.is_empty() || fits(&compact, 0) {
        return compact;
    }
    let variants: Vec<String> = e.variants.iter().map(print_variant_decl).collect();
    format!("{} {}", enum_head(e), wrap_delim("{", &variants, "}", 0))
}

fn trait_head(t: &TraitDecl) -> String {
    let pub_prefix = if t.is_pub { "pub " } else { "" };
    format!("{pub_prefix}trait {}", t.name)
}

fn print_trait(t: &TraitDecl) -> String {
    if t.methods.is_empty() {
        return format!("{} {{}}", trait_head(t));
    }
    let methods: Vec<String> = t.methods.iter().map(print_trait_method).collect();
    format!("{} {}", trait_head(t), wrap_brace("{", &methods, "}"))
}

fn print_trait_method(m: &TraitMethodSig) -> String {
    print_sig("", &m.name, &m.param, &print_type(&m.ret), INDENT)
}

fn impl_head(i: &ImplDecl) -> String {
    format!(
        "impl{} {} for {}",
        print_type_params(&i.params),
        i.trait_name,
        print_type(&i.ty)
    )
}

fn print_impl(i: &ImplDecl) -> String {
    if i.methods.is_empty() {
        return format!("{} {{}}", impl_head(i));
    }
    let methods: Vec<String> = i.methods.iter().map(print_impl_method).collect();
    format!("{} {}", impl_head(i), wrap_brace("{", &methods, "}"))
}

/// Sits one level in, inside the impl's braces, so that is where its own lines are measured
/// from and where a broken body indents from.
fn print_impl_method(m: &ImplMethod) -> String {
    let sig = print_sig("", &m.name, &m.param, &print_type(&m.ret), INDENT);
    print_signed(sig, &m.body, INDENT)
}

/// A brace block whose items need no separator between them (trait and impl methods each
/// start with their own `fn`, so a comma would break the next method's parse), laid out one
/// per line, indented one level.
fn wrap_brace(open: &str, items: &[String], close: &str) -> String {
    let mut out = format!("{open}\n");
    for item in items {
        out.push_str(&pad(INDENT));
        out.push_str(item);
        out.push('\n');
    }
    out.push_str(close);
    out
}

fn print_type(t: &TypeExpr) -> String {
    match t {
        TypeExpr::Named { name, args, .. } => {
            if args.is_empty() {
                name.clone()
            } else {
                let args_str = args.iter().map(print_type).collect::<Vec<_>>().join(", ");
                format!("{name}<{args_str}>")
            }
        }
        TypeExpr::Vec { elem, .. } => format!("Vec<{}>", print_type(elem)),
        TypeExpr::Stream { elem, .. } => format!("Stream<{}>", print_type(elem)),
        TypeExpr::Seq { head, rest, .. } => {
            format!("Seq<{}, {}>", print_type(head), print_type(rest))
        }
        TypeExpr::Record { fields, .. } => {
            let fields_str = fields
                .iter()
                .map(|(n, t)| format!("{n}: {}", print_type(t)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{fields_str}}}")
        }
    }
}

/// A closing delimiter's list never accepts a trailing comma (every list-parser in `parse.rs`
/// tries to read one more item right after a comma, with no check for an immediate close), so a
/// broken list must never write one before `close`.
fn wrap_delim(open: &str, items: &[String], close: &str, indent: usize) -> String {
    if items.is_empty() {
        return format!("{open}{close}");
    }
    let inner = indent + INDENT;
    let mut out = format!("{open}\n");
    for (i, item) in items.iter().enumerate() {
        out.push_str(&pad(inner));
        out.push_str(item);
        if i + 1 < items.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str(&pad(indent));
    out.push_str(close);
    out
}

fn escape_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// The base of a postfix chain (`Field`/`Index`/`Project`/`Unwrap`): only reachable bare via
/// `atom()`, so a compound child always needs parens there.
fn print_atom_base(base: &Expr) -> String {
    print_expr_compact(base, Ctx::Atom)
}

/// A `Call`/`Variant` argument is always printed inside real parens this module writes itself
/// (see the module doc), so the content resets to a fresh `Expr(0)` position.
fn print_paren_arg(e: &Expr) -> String {
    print_expr_compact(e, Ctx::Expr(0))
}

fn print_expr_compact(e: &Expr, ctx: Ctx) -> String {
    if needs_parens(e, ctx) {
        return format!("({})", print_expr_inner(e));
    }
    print_expr_inner(e)
}

/// A Float literal spelled so it lexes back as a Float. `float::lit` is the shortest
/// round-trip digits, which the backends want, but it drops the `.0` on a whole value
/// (`2.0` -> `2`) and writes `1e21` as a 22-digit run -- both of which the lexer reads as an
/// `Int`, so a formatted program changed type or stopped compiling (found by the docs
/// harness the day the Float reference page landed, 2026-09-18). Whole values keep a `.0`,
/// and values outside the printer's fixed-notation band (`1e-7` to `1e21`) use the exponent
/// form, which the lexer takes as a Float on its own.
fn float_literal(n: f64) -> String {
    let magnitude = n.abs();
    if magnitude >= 1e21 || (magnitude > 0.0 && magnitude < 1e-6) {
        return format!("{n:e}");
    }
    let plain = crate::float::lit(n);
    if plain.contains('.') {
        plain
    } else {
        format!("{plain}.0")
    }
}

/// The outer parens decision (`needs_parens`) has already been made by `print_expr_compact`;
/// every child position inside is fixed by its own node (a `Binary`'s own operator powers,
/// `Pipe`'s hard-coded `PIPE_RIGHT`, and so on), independent of the outer `ctx`.
fn print_expr_inner(e: &Expr) -> String {
    match e {
        Expr::Str { text, .. } => format!("\"{}\"", escape_str(text)),
        Expr::Int { value, .. } => value.to_string(),
        Expr::Float { value, .. } => float_literal(*value),
        Expr::VecLit { items, .. } => {
            let items_str = items
                .iter()
                .map(print_paren_arg)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{items_str}]")
        }
        Expr::RecordLit { fields, .. } => format!("{{{}}}", print_record_fields(fields)),
        Expr::Subject { .. } => ".".to_string(),
        Expr::Var { name, .. } => name.clone(),
        Expr::Call { func, arg, .. } => match arg {
            None => format!("{func}()"),
            Some(a) => format!("{func}({})", print_paren_arg(a)),
        },
        Expr::Project { base, .. } => format!("{}[]", print_atom_base(base)),
        Expr::Index { base, index, .. } => {
            format!("{}[{}]", print_atom_base(base), print_paren_arg(index))
        }
        Expr::Slice {
            base, start, end, ..
        } => {
            let lo = match start {
                Some(s) => print_paren_arg(s),
                None => String::new(),
            };
            let hi = match end {
                Some(e) => print_paren_arg(e),
                None => String::new(),
            };
            format!("{}[{lo}:{hi}]", print_atom_base(base))
        }
        Expr::Unwrap { base, .. } => format!("{}!", print_atom_base(base)),
        Expr::Neg { base, .. } => format!("-{}", print_expr_compact(base, Ctx::Unary)),
        // `not x`, not `not(x)`: this is an operator, and the parens spelling would read as the
        // call it is not. What follows is printed at `not`'s own power, so `not a == b` keeps
        // the parens off the comparison the parser would give it back.
        Expr::Not { base, .. } => {
            format!("not {}", print_expr_compact(base, Ctx::Operand(NOT_POWER)))
        }
        Expr::Field { base, name, .. } => {
            let base_str = print_atom_base(base);
            if base_str == "." {
                format!(".{name}")
            } else {
                format!("{base_str}.{name}")
            }
        }
        Expr::ColonCall {
            receiver,
            method,
            arg,
            ..
        } => {
            let base = print_atom_base(receiver);
            match arg {
                None => format!("{base}:{method}()"),
                Some(a) => format!("{base}:{method}({})", print_paren_arg(a)),
            }
        }
        Expr::Stdin { .. } => "stdin".to_string(),
        // `csv`/`tsv` are parser sugar, so the canonical spelling is the parameterized form.
        Expr::Dsv { delim, .. } => format!("dsv(\"{}\")", escape_str(delim)),
        Expr::Variant {
            enum_name,
            variant,
            payload,
            ..
        } => match payload {
            None => format!("{enum_name}.{variant}"),
            Some(p) => format!("{enum_name}.{variant}({})", print_paren_arg(p)),
        },
        Expr::Match { arms, .. } => arms
            .iter()
            .enumerate()
            .map(|(i, a)| print_match_arm(a, i + 1 == arms.len()))
            .collect::<Vec<_>>()
            .join(" or "),
        Expr::MatchCall {
            enum_name, arms, ..
        } => format!(
            "{enum_name}({})",
            arms.iter()
                .enumerate()
                .map(|(i, a)| print_match_arm(a, i + 1 == arms.len()))
                .collect::<Vec<_>>()
                .join(" or ")
        ),
        Expr::Pipe { lhs, rhs, .. } => {
            // `Pipe.lhs` accumulates the same way a `Binary` chain does (`a | b | c` folds
            // left, exactly like `a - b - c`), so a nested `Pipe` there reproduces itself with
            // no parens needed, the same as any other `Ctx::Expr(0)` position. Only `rhs` -- a
            // genuinely fresh `expr(PIPE_RIGHT)` call -- can force parens on a nested `Pipe`.
            format!(
                "{} | {}",
                print_expr_compact(lhs, Ctx::Expr(0)),
                print_expr_compact(rhs, Ctx::Expr(PIPE_RIGHT))
            )
        }
        Expr::TailPipe { lhs, callee, .. } => {
            // `|>` binds looser than everything and sits only at the outermost position, so the
            // lhs is a fresh `expr(0)` with no parens to reach the sink.
            format!("{} |> {callee}", print_expr_compact(lhs, Ctx::Expr(0)))
        }
        Expr::Binary { op, lhs, rhs, .. } => {
            let (left, right) = bin_power(*op);
            format!(
                "{} {op} {}",
                print_expr_compact(lhs, Ctx::Operand(left)),
                print_expr_compact(rhs, Ctx::Operand(right))
            )
        }
        Expr::Logic { op, lhs, rhs, .. } => {
            let (left, right) = logic_power(*op);
            format!(
                "{} {op} {}",
                print_expr_compact(lhs, Ctx::Operand(left)),
                print_expr_compact(rhs, Ctx::Operand(right))
            )
        }
        // `print_def` emits a `let` block before reaching this match; a `Let` here is a
        // nested one the parser never produces (the block form is only a function body).
        Expr::Let { .. } => unreachable!("a `let` block is only ever a function body"),
        Expr::ModuleRoute { path, .. } => format!("@(\"{}\")", escape_str(path)),
    }
}

fn print_record_fields(fields: &[(String, crate::ast::Span, Expr)]) -> String {
    fields
        .iter()
        .map(|(n, _, v)| format!("{n}: {}", print_paren_arg(v)))
        .collect::<Vec<_>>()
        .join(", ")
}

fn print_fields_pattern(f: &FieldsPattern) -> String {
    let names: Vec<&str> = f.names.iter().map(|(n, _)| n.as_str()).collect();
    if f.rest {
        if names.is_empty() {
            "..".to_string()
        } else {
            format!("{}, ..", names.join(", "))
        }
    } else {
        names.join(", ")
    }
}

/// A `Default` pattern comes from either an explicit `any()` or a bare trailing expression --
/// indistinguishable in the AST except that the bare form's span is exactly its body's span
/// (`parse.rs::guard_or_default_arm`). Printing bare whenever that holds and the arm is last
/// (the only place a bare default is legal) reproduces the shorter, and by construction always
/// idempotent, spelling; every other `Default` prints the explicit `any()` it must have been.
fn print_match_arm(arm: &MatchArm, is_last: bool) -> String {
    let body = print_expr_compact(&arm.body, Ctx::Expr(ARM_BODY));
    match arm_head(arm, is_last) {
        None => body,
        Some(head) => format!("{head} -> {body}"),
    }
}

/// What precedes the `->`, or `None` for a bare default arm, which is its body alone.
fn arm_head(arm: &MatchArm, is_last: bool) -> Option<String> {
    Some(match &arm.pattern {
        Pattern::Default { span } if is_last && *span == arm.body.span() => return None,
        Pattern::Default { .. } => "any()".to_string(),
        Pattern::Guard(g) => print_expr_compact(g, Ctx::Operand(COND_POWER)),
        Pattern::Variant { name, fields, .. } => match fields {
            None => name.clone(),
            Some(f) => format!("{name}{{{}}}", print_fields_pattern(f)),
        },
    })
}

/// An arm at `indent` that does not fit there (with the ` or` after it, when one follows)
/// breaks after its `->`, the body one level in, the same way a definition's body drops below
/// its signature; a bare default arm has no `->` and its body breaks in place.
fn print_match_arm_wrapped(arm: &MatchArm, is_last: bool, indent: usize, reserve: usize) -> String {
    let compact = print_match_arm(arm, is_last);
    let reserve = reserve + if is_last { 0 } else { " or".len() };
    if fits(&compact, indent + reserve) {
        return compact;
    }
    match arm_head(arm, is_last) {
        None => print_expr_fitting(&arm.body, Ctx::Expr(ARM_BODY), indent, reserve),
        Some(head) => {
            let inner = indent + INDENT;
            format!(
                "{head} ->\n{}{}",
                pad(inner),
                print_expr_fitting(&arm.body, Ctx::Expr(ARM_BODY), inner, reserve)
            )
        }
    }
}

/// Tries the compact form first; when it does not fit at `indent`, breaks the node at its own
/// natural seam (an operator, an arm, a delimited list) rather than leaving it overlong. Falls
/// back to the (overlong) compact form for node kinds with no seam to break at -- accepted
/// overflow, not a correctness problem, since every backend agrees on lines it never sees.
fn print_expr_wrapped(e: &Expr, ctx: Ctx, indent: usize) -> String {
    print_expr_fitting(e, ctx, indent, 0)
}

/// `print_expr_wrapped` with `reserve` columns held back on the node's last line for whatever
/// the caller appends there: a postfix suffix (`[n]!`), the trailing operator of a binary chain,
/// the ` or` after a match arm. Without it a base that fits by exactly its suffix's width is
/// left compact and the line overflows by that much.
fn print_expr_fitting(e: &Expr, ctx: Ctx, indent: usize, reserve: usize) -> String {
    let compact = print_expr_compact(e, ctx);
    if fits(&compact, indent + reserve) {
        return compact;
    }
    if needs_parens(e, ctx) {
        let inner = print_expr_wrapped(e, Ctx::Expr(0), indent + INDENT);
        return format!("(\n{}{inner}\n{})", pad(indent + INDENT), pad(indent));
    }
    let inner = indent + INDENT;
    match e {
        Expr::Binary { op, lhs, rhs, .. } => {
            let (left, right) = bin_power(*op);
            let op_str = op.to_string();
            let lhs_str = print_expr_fitting(lhs, Ctx::Operand(left), indent, op_str.len() + 1);
            let rhs_str = print_expr_fitting(rhs, Ctx::Operand(right), inner, reserve);
            format!("{lhs_str} {op_str}\n{}{rhs_str}", pad(inner))
        }
        Expr::Logic { op, lhs, rhs, .. } => {
            let (left, right) = logic_power(*op);
            let op_str = op.to_string();
            let lhs_str = print_expr_fitting(lhs, Ctx::Operand(left), indent, op_str.len() + 1);
            let rhs_str = print_expr_fitting(rhs, Ctx::Operand(right), inner, reserve);
            format!("{lhs_str} {op_str}\n{}{rhs_str}", pad(inner))
        }
        Expr::Not { base, .. } => format!(
            "not {}",
            print_expr_fitting(base, Ctx::Operand(NOT_POWER), indent, reserve)
        ),
        Expr::Pipe { .. } => wrap_pipe(e, indent, reserve),
        Expr::TailPipe { lhs, callee, .. } => {
            let tail = format!(" |> {callee}");
            format!(
                "{}{tail}",
                print_expr_fitting(lhs, Ctx::Expr(0), indent, tail.len() + reserve)
            )
        }
        Expr::Match { arms, .. } => wrap_match(arms, indent, reserve),
        Expr::Neg { base, .. } => {
            format!("-{}", print_expr_fitting(base, Ctx::Unary, indent, reserve))
        }
        _ => wrap_delimited(e, indent)
            .or_else(|| wrap_postfix(e, indent, reserve))
            // No natural seam to break at (`Var`, `Call`/`Variant` with no argument, and so
            // on): the compact form already computed above is the best available.
            .unwrap_or(compact),
    }
}

/// A node whose seam is a delimited list -- a literal, a call's argument -- opened one item per
/// line. `None` for every other node.
fn wrap_delimited(e: &Expr, indent: usize) -> Option<String> {
    let inner = indent + INDENT;
    Some(match e {
        Expr::VecLit { items, .. } => {
            let rendered: Vec<String> = items
                .iter()
                .map(|i| print_expr_wrapped(i, Ctx::Expr(0), inner))
                .collect();
            wrap_delim("[", &rendered, "]", indent)
        }
        Expr::RecordLit { fields, .. } => {
            let rendered: Vec<String> = fields
                .iter()
                .map(|(n, _, v)| format!("{n}: {}", print_expr_wrapped(v, Ctx::Expr(0), inner)))
                .collect();
            wrap_delim("{", &rendered, "}", indent)
        }
        Expr::Call {
            func, arg: Some(a), ..
        } => {
            let item = print_expr_wrapped(a, Ctx::Expr(0), inner);
            format!("{func}{}", wrap_delim("(", &[item], ")", indent))
        }
        Expr::Variant {
            enum_name,
            variant,
            payload: Some(p),
            ..
        } => {
            let item = print_expr_wrapped(p, Ctx::Expr(0), inner);
            format!(
                "{enum_name}.{variant}{}",
                wrap_delim("(", &[item], ")", indent)
            )
        }
        Expr::MatchCall {
            enum_name, arms, ..
        } => wrap_match_call(enum_name, arms, indent),
        Expr::ColonCall {
            receiver,
            method,
            arg: Some(a),
            ..
        } => {
            let item = print_expr_wrapped(a, Ctx::Expr(0), inner);
            format!(
                "{}:{method}{}",
                print_atom_base(receiver),
                wrap_delim("(", &[item], ")", indent)
            )
        }
        _ => return None,
    })
}

/// A postfix chain breaks inside its base, the suffix staying on the base's last line:
/// `[...][n]!` opens the list one item per line and closes it with `][n]!`. `None` for every
/// other node.
fn wrap_postfix(e: &Expr, indent: usize, reserve: usize) -> Option<String> {
    Some(match e {
        Expr::Index { base, index, .. } => {
            let suffix = format!("[{}]", print_paren_arg(index));
            format!(
                "{}{suffix}",
                print_postfix_base(base, indent, suffix.len() + reserve)
            )
        }
        Expr::Slice {
            base, start, end, ..
        } => {
            let lo = start
                .as_ref()
                .map(|s| print_paren_arg(s))
                .unwrap_or_default();
            let hi = end.as_ref().map(|e| print_paren_arg(e)).unwrap_or_default();
            let suffix = format!("[{lo}:{hi}]");
            format!(
                "{}{suffix}",
                print_postfix_base(base, indent, suffix.len() + reserve)
            )
        }
        Expr::Unwrap { base, .. } => format!("{}!", print_postfix_base(base, indent, 1 + reserve)),
        Expr::Project { base, .. } => {
            format!("{}[]", print_postfix_base(base, indent, 2 + reserve))
        }
        // `.name` (a `Subject` base) is already as short as it gets and is the compact form.
        Expr::Field { base, name, .. } if !matches!(**base, Expr::Subject { .. }) => {
            let suffix = format!(".{name}");
            format!(
                "{}{suffix}",
                print_postfix_base(base, indent, suffix.len() + reserve)
            )
        }
        _ => return None,
    })
}

fn print_postfix_base(base: &Expr, indent: usize, reserve: usize) -> String {
    print_expr_fitting(base, Ctx::Atom, indent, reserve)
}

/// Flattens a left-recursive `Pipe` chain (`a | b | c` parses as `(a | b) | c`, the same fold
/// `Binary` chains use) into its stages in source order.
fn flatten_pipe(e: &Expr) -> Vec<&Expr> {
    let mut stages = Vec::new();
    let mut cur = e;
    while let Expr::Pipe { lhs, rhs, .. } = cur {
        stages.push(rhs.as_ref());
        cur = lhs.as_ref();
    }
    stages.push(cur);
    stages.reverse();
    stages
}

/// A pipeline that does not fit breaks one stage per line, with `|` as the first character of
/// every continuation line so the pipes line up in a vertical column (issue #101) -- unlike
/// `Binary`'s trailing-operator rule, which a bare two-operand `Pipe` no longer follows.
fn wrap_pipe(e: &Expr, indent: usize, reserve: usize) -> String {
    let stages = flatten_pipe(e);
    let mut out = print_expr_wrapped(stages[0], Ctx::Expr(0), indent);
    let last = stages.len() - 1;
    for (i, stage) in stages.iter().enumerate().skip(1) {
        // The stage's budget must include the two columns "| " occupies, or a stage that
        // just fits alone overruns the line by exactly that prefix -- and a stage that
        // wraps internally hangs its closing bracket left of its own opener.
        let reserve = if i == last { reserve } else { 0 };
        let stage_str =
            print_expr_fitting(stage, Ctx::Expr(PIPE_RIGHT), indent + INDENT + 2, reserve);
        out.push('\n');
        out.push_str(&pad(indent + INDENT));
        out.push_str("| ");
        out.push_str(&stage_str);
    }
    out
}

/// The first arm stays at `indent`, where the chain began; every later arm sits one level in.
fn wrap_match(arms: &[MatchArm], indent: usize, reserve: usize) -> String {
    let n = arms.len();
    let mut out = String::new();
    for (i, a) in arms.iter().enumerate() {
        let column = if i == 0 { indent } else { indent + INDENT };
        if i > 0 {
            out.push_str(" or\n");
            out.push_str(&pad(column));
        }
        let reserve = if i + 1 == n { reserve } else { 0 };
        out.push_str(&print_match_arm_wrapped(a, i + 1 == n, column, reserve));
    }
    out
}

/// The wrapped form of a match call (gh:152): the arms open the parens one level down from the
/// head, separated by `or` the way a `Match`'s are, so a call form that does not fit breaks at
/// its arms rather than overflowing one line.
fn wrap_match_call(enum_name: &str, arms: &[MatchArm], indent: usize) -> String {
    let n = arms.len();
    let inner = indent + INDENT;
    let mut out = format!("{enum_name}(\n");
    for (i, a) in arms.iter().enumerate() {
        out.push_str(&pad(inner));
        out.push_str(&print_match_arm_wrapped(a, i + 1 == n, inner, 0));
        if i + 1 < n {
            out.push_str(" or\n");
        } else {
            out.push('\n');
        }
    }
    out.push_str(&pad(indent));
    out.push(')');
    out
}
