//! The one-line template, and every node's single-line rendering. `print_expr_compact` is what
//! the file template tries before breaking a node; `emit_one_line` joins the same renderings
//! into one line for the whole program. Nothing here knows about width or comments.
//!
//! The grammar reads a line break as the separator between a `let` value and the block's value,
//! and between the last definition's body and the program body; on one line, a body ending in
//! a name followed by an atom re-reads as a bare application (`x g(1)` is `x(g(1))`). This
//! module does not guard against that: `format_one_line` refuses the program when the reading
//! changed.

use super::parens::{
    ARM_BODY, COND_POWER, Ctx, NOT_POWER, PIPE_RIGHT, bin_power, logic_power, needs_parens,
};
use super::{Item, decls_in_source_order};
use crate::ast::{
    Alias, Def, EnumDecl, Expr, FieldsPattern, File, ImplDecl, ImplMethod, MatchArm, Module, Param,
    ParamShape, Pattern, TraitDecl, TraitMethodSig, TypeExpr, Variant,
};

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

pub(super) fn print_alias(a: &Alias) -> String {
    format!("type {} = {}", a.name, print_type(&a.ty))
}

pub(super) fn print_param(p: &Option<Param>) -> String {
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

pub(super) fn print_variant_decl(v: &Variant) -> String {
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

pub(super) fn enum_head(e: &EnumDecl) -> String {
    let pub_prefix = if e.is_pub { "pub " } else { "" };
    format!(
        "{pub_prefix}enum {}{}",
        e.name,
        print_type_params(&e.params)
    )
}

pub(super) fn print_enum_compact(e: &EnumDecl) -> String {
    let variants: Vec<String> = e.variants.iter().map(print_variant_decl).collect();
    if variants.is_empty() {
        return format!("{} {{}}", enum_head(e));
    }
    format!("{} {{ {} }}", enum_head(e), variants.join(", "))
}

pub(super) fn trait_head(t: &TraitDecl) -> String {
    let pub_prefix = if t.is_pub { "pub " } else { "" };
    format!("{pub_prefix}trait {}", t.name)
}

pub(super) fn print_trait_method(m: &TraitMethodSig) -> String {
    format!(
        "fn {}({}) -> {}",
        m.name,
        print_param(&m.param),
        print_type(&m.ret)
    )
}

pub(super) fn impl_head(i: &ImplDecl) -> String {
    format!(
        "impl{} {} for {}",
        print_type_params(&i.params),
        i.trait_name,
        print_type(&i.ty)
    )
}

pub(super) fn print_type(t: &TypeExpr) -> String {
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
pub(super) fn print_atom_base(base: &Expr) -> String {
    print_expr_compact(base, Ctx::Atom)
}

/// A `Call`/`Variant` argument is always printed inside real parens this module writes itself
/// (see the module doc), so the content resets to a fresh `Expr(0)` position.
pub(super) fn print_paren_arg(e: &Expr) -> String {
    print_expr_compact(e, Ctx::Expr(0))
}

pub(super) fn print_expr_compact(e: &Expr, ctx: Ctx) -> String {
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
/// `- -5`, not `--5`: the parser reads both as two negations, but the second reads as a
/// decrement to a person.
pub(super) fn neg_sign(base: &Expr) -> &'static str {
    if matches!(base, Expr::Neg { .. }) {
        "- "
    } else {
        "-"
    }
}

fn neg(base: &Expr) -> String {
    format!("{}{}", neg_sign(base), print_expr_compact(base, Ctx::Unary))
}

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
        Expr::Neg { base, .. } => neg(base),
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

pub(super) fn print_fields_pattern(f: &FieldsPattern) -> String {
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
pub(super) fn print_match_arm(arm: &MatchArm, is_last: bool) -> String {
    let body = print_expr_compact(&arm.body, Ctx::Expr(ARM_BODY));
    match arm_head(arm, is_last) {
        None => body,
        Some(head) => format!("{head} -> {body}"),
    }
}

/// What precedes the `->`, or `None` for a bare default arm, which is its body alone.
pub(super) fn arm_head(arm: &MatchArm, is_last: bool) -> Option<String> {
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
