//! The file template. A function whose signature-plus-body fits on one line stays on one line;
//! otherwise the body moves to its own indented line, and a binary chain that still does not
//! fit breaks at its outermost operator, trailing the operator on the first line. Match-arm
//! chains extend that same "trailing operator, one extra indent" rule, with a broken arm body
//! one level below the later arms' column. A pipeline that does not fit is the one exception:
//! it breaks one stage per line, with `|` leading each continuation line so the pipes form a
//! vertical column (maintainer directive, issue #101). Every printer here tries the node's
//! one-line rendering (`one_line`) first and breaks only when it does not fit at its column,
//! with `reserve` columns held back for whatever the caller appends on the last line.

use super::comments::{Comments, comment_lines, with_trailing};
use super::one_line::{
    arm_head, enum_head, impl_head, print_alias, print_atom_base, print_enum_compact,
    print_expr_compact, print_fields_pattern, print_match_arm, print_param, print_paren_arg,
    print_type, print_variant_decl, trait_head,
};
use super::parens::{ARM_BODY, Ctx, NOT_POWER, PIPE_RIGHT, bin_power, logic_power, needs_parens};
use super::{Item, decls_in_source_order};
use crate::ast::{
    Def, EnumDecl, Expr, File, ImplDecl, ImplMethod, MatchArm, Module, Param, ParamShape,
    TraitDecl, TraitMethodSig, TypeExpr,
};

const WIDTH: usize = 80;

pub(super) const INDENT: usize = 4;

fn fits(s: &str, indent: usize) -> bool {
    indent + s.chars().count() <= WIDTH
}

pub(super) fn pad(n: usize) -> String {
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

/// A comment's `blank_after` can leave a blank line at the very end; a file ends in exactly one
/// newline.
fn ensure_single_newline(out: String) -> String {
    format!("{}\n", out.trim_end())
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

/// `fn name(param) -> ret`, on one line when it and the ` =` after it fit at `indent`. Otherwise
/// the parameter goes on its own line one level in, and a record parameter type that still does
/// not fit there breaks one field per line. A nullary signature has nothing to break at.
pub(super) fn print_sig(
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

pub(super) fn print_def(d: &Def) -> String {
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

fn print_enum(e: &EnumDecl) -> String {
    let compact = print_enum_compact(e);
    if e.variants.is_empty() || fits(&compact, 0) {
        return compact;
    }
    let variants: Vec<String> = e.variants.iter().map(print_variant_decl).collect();
    format!("{} {}", enum_head(e), wrap_delim("{", &variants, "}", 0))
}

fn print_trait(t: &TraitDecl) -> String {
    if t.methods.is_empty() {
        return format!("{} {{}}", trait_head(t));
    }
    let methods: Vec<String> = t.methods.iter().map(print_trait_method).collect();
    format!("{} {}", trait_head(t), wrap_brace("{", &methods, "}"))
}

/// Sits one level in, inside the trait's braces, so a signature that breaks measures and
/// indents from there.
fn print_trait_method(m: &TraitMethodSig) -> String {
    print_sig("", &m.name, &m.param, &print_type(&m.ret), INDENT)
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

/// An arm at `indent` that does not fit there (with the ` or` after it, when one follows)
/// breaks after its `->`, its body dropping to `body_column`: one level below the column the
/// chain's continuation arms sit at, so a broken body is always visibly deeper than the arm
/// that follows it (maintainer ruling, 2026-09-19). A bare default arm has no `->` and its
/// body breaks in place.
fn print_match_arm_wrapped(
    arm: &MatchArm,
    is_last: bool,
    indent: usize,
    body_column: usize,
    reserve: usize,
) -> String {
    let compact = print_match_arm(arm, is_last);
    let reserve = reserve + if is_last { 0 } else { " or".len() };
    if fits(&compact, indent + reserve) {
        return compact;
    }
    match arm_head(arm, is_last) {
        None => print_expr_fitting(&arm.body, Ctx::Expr(ARM_BODY), indent, reserve),
        Some(head) => format!(
            "{head} ->\n{}{}",
            pad(body_column),
            print_expr_fitting(&arm.body, Ctx::Expr(ARM_BODY), body_column, reserve)
        ),
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
        let body_column = indent + 2 * INDENT;
        out.push_str(&print_match_arm_wrapped(
            a,
            i + 1 == n,
            column,
            body_column,
            reserve,
        ));
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
        out.push_str(&print_match_arm_wrapped(
            a,
            i + 1 == n,
            inner,
            inner + INDENT,
            0,
        ));
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
