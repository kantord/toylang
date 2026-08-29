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
//! does not fit breaks at its outermost operator, trailing the operator on the first line. Pipe
//! chains, match-arm chains, and conditional chains extend that same "trailing operator, one
//! extra indent" rule by analogy, since nothing in the sample pins their layout directly.
//!
//! Two style choices worth naming since nothing in the grammar forces them: calls are always
//! written with explicit parens (`f(x)`, never the bare `f x` or brace-shorthand `f{...}`),
//! matching both calls in the sample; and the wrap width is 80 columns, backed out from the
//! sample itself -- the one line it left alone is 44 columns, and the two it broke are 89 and
//! 118.
//!
//! One thing the AST cannot carry: comments. `parse::parse` throws every `#...` away as trivia
//! (`skip_trivia`), so `emit` alone can only ever produce a program with none. `format_source`
//! covers the one shape that actually appears in this repository -- a file's leading banner,
//! read back off the raw text rather than the tree -- and nothing else; a comment anywhere else
//! does not survive formatting, which is why every fragment that puts one somewhere else is
//! marked a syntax-spelling exception rather than swept.

use crate::ast::{
    Alias, BinOp, Def, EnumDecl, Expr, FieldsPattern, File, MatchArm, Param, Pattern, TypeExpr,
    Variant,
};

const WIDTH: usize = 80;
const INDENT: usize = 4;

// Mirrors parse.rs's precedence table exactly (`infix_power`, `PIPE_LEFT`/`PIPE_RIGHT`,
// `COND_POWER`): the numbers a paren decision here has to answer to are the parser's, not this
// module's own invention.
const PIPE_LEFT: u8 = 1;
const PIPE_RIGHT: u8 = 2;
const COND_POWER: u8 = 3;

fn bin_power(op: BinOp) -> (u8, u8) {
    match op {
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => (5, 6),
        BinOp::Add | BinOp::Sub => (7, 8),
        BinOp::Mul | BinOp::Div | BinOp::Rem => (9, 10),
    }
}

/// Where a child expression sits, in terms of what the parser would have accepted bare at that
/// spot -- everything `needs_parens` and the wrapping printer need to know to reproduce the tree
/// exactly, and nothing else.
#[derive(Clone, Copy)]
enum Ctx {
    /// A fresh `self.expr(m)` call: File/Def body, a call argument (always real parens here, so
    /// always reset to `Expr(0)` inside them), `Index`'s bracketed expression, a match arm's
    /// body, or `Cond::otherwise`. The one context where a bare `Cond` and, when `m` is loose
    /// enough, a bare `Pipe` or `Match` are all reachable.
    Expr(u8),
    /// A `self.operand(m)` call: a `Binary` child, or `Pattern::Guard`'s expression. `Cond`,
    /// `Pipe`, and `Match` are never reachable bare here -- only `expr()` parses those.
    Operand(u8),
    /// `Cond::then`: built the same way as `Operand(m)`, except a `Match` is also reachable
    /// (bare, when `m <= PIPE_RIGHT`) because it is built by the same call that would otherwise
    /// have produced the `Operand(m)` result, before the `if` is even seen.
    CondThen(u8),
    /// The base of a postfix chain (`Field`/`Index`/`Project`/`Unwrap`), or a `Call`/`Variant`'s
    /// argument print used only when NOT already inside real parens. Nothing compound is
    /// reachable bare here -- not even `Neg`.
    Atom,
    /// `Neg`'s base: like `Atom`, except a nested `Neg` is reachable bare (`- -a`).
    Unary,
}

fn needs_parens(e: &Expr, ctx: Ctx) -> bool {
    match ctx {
        Ctx::Atom => matches!(
            e,
            Expr::Binary { .. }
                | Expr::Pipe { .. }
                | Expr::Cond { .. }
                | Expr::Match { .. }
                | Expr::Neg { .. }
        ),
        Ctx::Unary => {
            matches!(
                e,
                Expr::Binary { .. } | Expr::Pipe { .. } | Expr::Cond { .. } | Expr::Match { .. }
            )
        }
        Ctx::Operand(m) => match e {
            Expr::Binary { op, .. } => bin_power(*op).0 < m,
            Expr::Pipe { .. } | Expr::Cond { .. } | Expr::Match { .. } => true,
            _ => false,
        },
        Ctx::CondThen(m) => match e {
            Expr::Binary { op, .. } => bin_power(*op).0 < m,
            Expr::Match { .. } => m > PIPE_RIGHT,
            Expr::Pipe { .. } | Expr::Cond { .. } => true,
            _ => false,
        },
        Ctx::Expr(m) => match e {
            Expr::Binary { op, .. } => bin_power(*op).0 < m,
            Expr::Pipe { .. } => PIPE_LEFT < m,
            Expr::Match { .. } => m > PIPE_RIGHT,
            _ => false,
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
    enum Item<'a> {
        Alias(&'a Alias),
        Enum(&'a EnumDecl),
        Def(&'a Def),
    }

    // The AST groups declarations by kind, losing their interleaving in the source; sorting by
    // span start puts them back in source order, which is what makes the output idempotent --
    // reformatting an already-sorted file is a no-op re-sort.
    let mut items: Vec<(usize, Item)> = Vec::new();
    for a in &file.aliases {
        items.push((a.span.start, Item::Alias(a)));
    }
    for e in &file.enums {
        items.push((e.span.start, Item::Enum(e)));
    }
    for d in &file.defs {
        items.push((d.span.start, Item::Def(d)));
    }
    items.sort_by_key(|(start, _)| *start);

    let mut out = String::new();
    for (_, item) in &items {
        let rendered = match item {
            Item::Alias(a) => print_alias(a),
            Item::Enum(e) => print_enum(e),
            Item::Def(d) => print_def(d),
        };
        out.push_str(&rendered);
        out.push_str("\n\n");
    }
    out.push_str(&print_expr_wrapped(&file.body, Ctx::Expr(0), 0));
    out.push('\n');
    out
}

/// Parses `src` and formats it, then reattaches the one piece of source text `emit` cannot
/// reconstruct from the tree: a leading run of `#` comment (and blank) lines, copied back
/// verbatim ahead of the formatted body.
pub fn format_source(src: &str) -> Result<String, crate::error::Error> {
    let file = crate::parse::parse(src)?;
    Ok(format!("{}{}", leading_comment(src), emit(&file)))
}

fn leading_comment(src: &str) -> String {
    let mut banner = String::new();
    for line in src.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            banner.push_str(line.trim_end());
            banner.push('\n');
        } else if trimmed.is_empty() {
            if banner.is_empty() {
                continue;
            }
            break;
        } else {
            break;
        }
    }
    banner
}

fn print_alias(a: &Alias) -> String {
    format!("type {} = {}", a.name, print_type(&a.ty))
}

fn print_param(p: &Option<Param>) -> String {
    match p {
        None => String::new(),
        Some(p) => format!("{}: {}", p.name, print_type(&p.ty)),
    }
}

fn print_def(d: &Def) -> String {
    let pub_prefix = if d.is_pub { "pub " } else { "" };
    let sig = format!(
        "{pub_prefix}fn {}({}) -> {}",
        d.name,
        print_param(&d.param),
        print_type(&d.ret)
    );
    let compact_body = print_expr_compact(&d.body, Ctx::Expr(0));
    let one_line = format!("{sig} = {compact_body}");
    if fits(&one_line, 0) {
        return one_line;
    }
    let body = print_expr_wrapped(&d.body, Ctx::Expr(0), INDENT);
    format!("{sig} =\n{}{body}", pad(INDENT))
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

fn print_enum(e: &EnumDecl) -> String {
    let pub_prefix = if e.is_pub { "pub " } else { "" };
    let params = if e.params.is_empty() {
        String::new()
    } else {
        let names: Vec<&str> = e.params.iter().map(|(n, _)| n.as_str()).collect();
        format!("<{}>", names.join(", "))
    };
    let head = format!("{pub_prefix}enum {}{params}", e.name);
    let variants: Vec<String> = e.variants.iter().map(print_variant_decl).collect();
    if variants.is_empty() {
        return format!("{head} {{}}");
    }
    let compact = format!("{head} {{ {} }}", variants.join(", "));
    if fits(&compact, 0) {
        return compact;
    }
    format!("{head} {}", wrap_delim("{", &variants, "}", 0))
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
        return format!("({})", print_expr_inner(e, ctx));
    }
    print_expr_inner(e, ctx)
}

/// The outer parens decision (`needs_parens`) has already been made by `print_expr_compact`;
/// `ctx` is threaded in only for `Cond::then`, which is the one child position whose available
/// bare kinds depend on where the *whole* `Cond` sits. Every other child position is fixed by
/// its own node (a `Binary`'s own operator powers, `Pipe`'s hard-coded `PIPE_RIGHT`, and so on),
/// independent of `ctx`.
fn print_expr_inner(e: &Expr, ctx: Ctx) -> String {
    match e {
        Expr::Str { text, .. } => format!("\"{}\"", escape_str(text)),
        Expr::Int { value, .. } => value.to_string(),
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
        Expr::Unwrap { base, .. } => format!("{}!", print_atom_base(base)),
        Expr::Neg { base, .. } => format!("-{}", print_expr_compact(base, Ctx::Unary)),
        Expr::Cond {
            then,
            cond,
            otherwise,
            ..
        } => {
            let m = outer_m(ctx);
            format!(
                "{} if {} else {}",
                print_expr_compact(then, Ctx::CondThen(m)),
                print_expr_compact(cond, Ctx::Operand(COND_POWER + 1)),
                print_expr_compact(otherwise, Ctx::Expr(COND_POWER))
            )
        }
        Expr::Field { base, name, .. } => {
            let base_str = print_atom_base(base);
            if base_str == "." {
                format!(".{name}")
            } else {
                format!("{base_str}.{name}")
            }
        }
        Expr::Input { .. } => "input".to_string(),
        Expr::Inputs { .. } => "inputs".to_string(),
        Expr::Lines { .. } => "lines".to_string(),
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
        Expr::Binary { op, lhs, rhs, .. } => {
            let (left, right) = bin_power(*op);
            format!(
                "{} {op} {}",
                print_expr_compact(lhs, Ctx::Operand(left)),
                print_expr_compact(rhs, Ctx::Operand(right))
            )
        }
    }
}

fn outer_m(ctx: Ctx) -> u8 {
    match ctx {
        Ctx::Expr(m) | Ctx::Operand(m) | Ctx::CondThen(m) => m,
        Ctx::Atom | Ctx::Unary => 0,
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
    let body = print_expr_compact(&arm.body, Ctx::Expr(COND_POWER));
    match &arm.pattern {
        Pattern::Default { span } if is_last && *span == arm.body.span() => body,
        Pattern::Default { .. } => format!("any() -> {body}"),
        Pattern::Guard(g) => format!(
            "{} -> {body}",
            print_expr_compact(g, Ctx::Operand(COND_POWER))
        ),
        Pattern::Variant { name, fields, .. } => {
            let head = match fields {
                None => name.clone(),
                Some(f) => format!("{name}{{{}}}", print_fields_pattern(f)),
            };
            format!("{head} -> {body}")
        }
    }
}

/// Tries the compact form first; when it does not fit at `indent`, breaks the node at its own
/// natural seam (an operator, an arm, a delimited list) rather than leaving it overlong. Falls
/// back to the (overlong) compact form for node kinds with no seam to break at -- accepted
/// overflow, not a correctness problem, since every backend agrees on lines it never sees.
fn print_expr_wrapped(e: &Expr, ctx: Ctx, indent: usize) -> String {
    let compact = print_expr_compact(e, ctx);
    if fits(&compact, indent) {
        return compact;
    }
    if needs_parens(e, ctx) {
        let inner = print_expr_wrapped(e, Ctx::Expr(0), indent + INDENT);
        return format!("(\n{}{inner}\n{})", pad(indent + INDENT), pad(indent));
    }
    match e {
        Expr::Binary { op, lhs, rhs, .. } => {
            let (left, right) = bin_power(*op);
            let lhs_str = print_expr_wrapped(lhs, Ctx::Operand(left), indent);
            let rhs_str = print_expr_wrapped(rhs, Ctx::Operand(right), indent + INDENT);
            format!("{lhs_str} {op}\n{}{rhs_str}", pad(indent + INDENT))
        }
        Expr::Pipe { lhs, rhs, .. } => {
            let lhs_str = print_expr_wrapped(lhs, Ctx::Expr(0), indent);
            let rhs_str = print_expr_wrapped(rhs, Ctx::Expr(PIPE_RIGHT), indent + INDENT);
            format!("{lhs_str} |\n{}{rhs_str}", pad(indent + INDENT))
        }
        Expr::Match { arms, .. } => wrap_match(arms, indent),
        Expr::Cond { .. } => wrap_cond(e, outer_m(ctx), indent),
        Expr::VecLit { items, .. } => {
            let rendered: Vec<String> = items
                .iter()
                .map(|i| print_expr_wrapped(i, Ctx::Expr(0), indent + INDENT))
                .collect();
            wrap_delim("[", &rendered, "]", indent)
        }
        Expr::RecordLit { fields, .. } => {
            let rendered: Vec<String> = fields
                .iter()
                .map(|(n, _, v)| {
                    format!(
                        "{n}: {}",
                        print_expr_wrapped(v, Ctx::Expr(0), indent + INDENT)
                    )
                })
                .collect();
            wrap_delim("{", &rendered, "}", indent)
        }
        Expr::Call {
            func, arg: Some(a), ..
        } => {
            let item = print_expr_wrapped(a, Ctx::Expr(0), indent + INDENT);
            format!("{func}{}", wrap_delim("(", &[item], ")", indent))
        }
        Expr::Variant {
            enum_name,
            variant,
            payload: Some(p),
            ..
        } => {
            let item = print_expr_wrapped(p, Ctx::Expr(0), indent + INDENT);
            format!(
                "{enum_name}.{variant}{}",
                wrap_delim("(", &[item], ")", indent)
            )
        }
        Expr::Neg { base, .. } => format!("-{}", print_expr_wrapped(base, Ctx::Unary, indent)),
        // No natural seam to break at (`Var`, `Call`/`Variant` with no argument, a projection or
        // field chain, and so on): the compact form already computed above is the best available.
        _ => compact,
    }
}

fn wrap_match(arms: &[MatchArm], indent: usize) -> String {
    let n = arms.len();
    let mut parts = arms
        .iter()
        .enumerate()
        .map(|(i, a)| print_match_arm(a, i + 1 == n));
    let mut out = parts.next().expect("a match always has at least one arm");
    for p in parts {
        out.push_str(" or\n");
        out.push_str(&pad(indent + INDENT));
        out.push_str(&p);
    }
    out
}

/// Flattens a right-recursive `Cond` chain (`a if c else b if d else e`) into its branches and
/// final default, the same shape `wrap_match` handles for an arm chain: `if`/`else` chains and
/// `or` chains are printed identically for the reason described where each is called.
fn flatten_cond(e: &Expr) -> (Vec<(&Expr, &Expr)>, &Expr) {
    let mut branches = Vec::new();
    let mut cur = e;
    while let Expr::Cond {
        then,
        cond,
        otherwise,
        ..
    } = cur
    {
        branches.push((then.as_ref(), cond.as_ref()));
        cur = otherwise.as_ref();
    }
    (branches, cur)
}

fn wrap_cond(e: &Expr, m: u8, indent: usize) -> String {
    let (branches, final_otherwise) = flatten_cond(e);
    let mut lines: Vec<String> = branches
        .iter()
        .enumerate()
        .map(|(i, (then, cond))| {
            // The first branch's `then` sits wherever the whole `Cond` sits; every later branch
            // is reached only through `Cond::otherwise`, which is always an `Expr(COND_POWER)`
            // position, so its own `then` is always `CondThen(COND_POWER)`.
            let then_m = if i == 0 { m } else { COND_POWER };
            format!(
                "{} if {} else",
                print_expr_compact(then, Ctx::CondThen(then_m)),
                print_expr_compact(cond, Ctx::Operand(COND_POWER + 1))
            )
        })
        .collect();
    lines.push(print_expr_wrapped(
        final_otherwise,
        Ctx::Expr(COND_POWER),
        indent + INDENT,
    ));

    let mut out = lines[0].clone();
    for line in &lines[1..] {
        out.push('\n');
        out.push_str(&pad(indent + INDENT));
        out.push_str(line);
    }
    out
}
