use crate::ast::BinOp;
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::Type;

/// The binding the input value is read into. Unspellable in source, since every source name is
/// prefixed.
const INPUT: &str = "t_input";
const INPUTS: &str = "t_inputs";

const SELECT_HELPER: &str = "\
function tl_select(src, pred) {
  const out = [];
  for (let i = 0; i < src.length; i++) if (pred(src[i])) out.push(src[i]);
  return out;
}
";

const FIELD_HELPER: &str = "\
function tl_field(v, k, depth) {
  if (depth === 0) return v[k];
  const out = [];
  for (let i = 0; i < v.length; i++) out[i] = tl_field(v[i], k, depth - 1);
  return out;
}
";

const OPT_HELPER: &str = "\
// An Opt is its enum's own runtime shape (ADR 0009): `{some: v}` present, \"none\" absent.
// Tagged, so two levels of absence stay two values; only the printer flattens to null.
function tl_at(v, i, depth) {
  if (depth > 0) return v.map((e) => tl_at(e, i, depth - 1));
  const n = v.length;
  if (i < 0) i = n + i;
  if (i < 0 || i >= n) return \"none\";
  return { some: v[i] };
}
";

const TAIL_HELPER: &str = "\
function tl_tail(v) {
  if (v.length === 0) return \"none\";
  return { some: v.slice(1) };
}
";

const UNWRAP_HELPER: &str = r#"function tl_unwrap(v, depth) {
  if (depth > 0) return v.map((e) => tl_unwrap(e, depth - 1));
  if (v === "none") { throw new Error("toylang: unwrapped a value that is not there"); }
  return v.some;
}
"#;

const ARITH_HELPER: &str = r#"// `|0` is ToInt32: it wraps to 32 bits and truncates toward zero, and V8 folds it away once it
// knows the value is already a Smi.
function tl_div(a, b) {
  if (b === 0) { throw new Error("toylang: divided by zero"); }
  return (a / b) | 0;
}
function tl_rem(a, b) {
  if (b === 0) { throw new Error("toylang: divided by zero"); }
  return (a % b) | 0;
}
"#;

const COLLECT_HELPER: &str = r#"// Synchronous, because a toylang expression evaluates to completion and node has no
// synchronous line reader built in. Reads in fixed chunks off the real fd rather than
// `readFileSync(0)`, so a line is available to the rest of the program as soon as it arrives
// rather than only once stdin closes.
function tl_collect_lines() {
  const fs = require("fs");
  const out = [];
  let buf = "";
  const chunk = Buffer.alloc(65536);
  for (;;) {
    const n = fs.readSync(0, chunk, 0, chunk.length, null);
    if (n === 0) break;
    buf += chunk.toString("utf8", 0, n);
    let i;
    while ((i = buf.indexOf("\n")) !== -1) {
      out.push(buf.slice(0, i));
      buf = buf.slice(i + 1);
    }
  }
  if (buf.length > 0) out.push(buf);
  return out;
}
"#;

const JOIN_HELPER: &str = "\
function tl_join(v, f) {
  const parts = [];
  for (let i = 0; i < v.length; i++) parts.push(f(v[i]));
  return \"[\" + parts.join(\",\") + \"]\";
}
";

// JS string comparison compares UTF-16 code units, which disagrees with codepoint order on any
// pair straddling a surrogate pair: a BMP character above U+DFFF (e.g. U+E000) sorts below an
// astral character's lead surrogate even though its codepoint is smaller. Iterating with the
// string iterator (rather than indexing) steps one codepoint at a time, surrogate pairs included.
const STR_CMP_HELPER: &str = "\
function tl_str_cmp(a, b) {
  const ai = a[Symbol.iterator]();
  const bi = b[Symbol.iterator]();
  for (;;) {
    const x = ai.next();
    const y = bi.next();
    if (x.done || y.done) return (x.done ? 0 : 1) - (y.done ? 0 : 1);
    const cx = x.value.codePointAt(0);
    const cy = y.value.codePointAt(0);
    if (cx !== cy) return cx < cy ? -1 : 1;
  }
}
";

const JSONLINES_HELPER: &str = "\
function tl_jsonlines(v, f) {
  const parts = [];
  for (let i = 0; i < v.length; i++) parts.push(f(v[i]));
  return parts.join(\"\\n\");
}
";

pub fn emit(program: &Program) -> String {
    let mut out = String::new();

    let used = used_helpers(program);
    if used.select {
        out.push_str(SELECT_HELPER);
    }
    if used.field {
        out.push_str(FIELD_HELPER);
    }
    if matches!(program.body.ty, Type::Vec(_)) || contains_vec(&program.body.ty) || used.jsonlines {
        out.push_str(JOIN_HELPER);
    }
    if used.index {
        out.push_str(OPT_HELPER);
    }
    if used.tail {
        out.push_str(TAIL_HELPER);
    }
    if used.unwrap {
        out.push_str(UNWRAP_HELPER);
    }
    if used.arith {
        out.push_str(ARITH_HELPER);
    }
    if used.collect {
        out.push_str(COLLECT_HELPER);
    }
    if used.jsonlines {
        out.push_str(JSONLINES_HELPER);
    }
    if used.str_cmp {
        out.push_str(STR_CMP_HELPER);
    }

    // Function declarations hoist, so a call to one defined further down resolves without the
    // forward declarations Lua needs. Each backend does what its own target does.
    for f in &program.funcs {
        out.push_str(&format!(
            "function {}({}) {{\n  return {};\n}}\n",
            user(&f.name),
            f.param.as_deref().map_or_else(String::new, user),
            expr(&f.body)
        ));
    }

    if let Some(fusion) = tir::fusion(program) {
        out.push_str(&fused_main(program, &fusion));
        return out;
    }

    if program.input.is_some() {
        out.push_str(&format!(
            "const {INPUT} = JSON.parse(require(\"fs\").readFileSync(0, \"utf8\"));\n"
        ));
    }
    if program.inputs.is_some() {
        out.push_str(&format!(
            "const {INPUTS} = require(\"fs\").readFileSync(0, \"utf8\").split(\"\\n\").filter((l) => l.length > 0).map((l) => JSON.parse(l));\n"
        ));
    }

    let body = expr(&program.body);
    // A top-level Str prints raw, the way jq's -r does; anything else prints as JSON.
    if program.body.ty == Type::Str {
        out.push_str(&format!("console.log({body});\n"));
    } else {
        out.push_str(&format!(
            "console.log({});\n",
            show(&program.body.ty, &body, 0)
        ));
    }
    out
}

/// A stream-typed `jsonlines` program, compiled as a loop reading one line at a time off the
/// real fd rather than `readFileSync(0)`'s read-everything-first.
///
/// `process.stdout.write` is synchronous to a file or TTY on POSIX but asynchronous to a pipe --
/// queued rather than issued immediately -- and the very next thing this loop does is a
/// *synchronous* read that blocks the whole event loop, which risks stopping a queued write from
/// ever reaching the pipe before the process blocks again. `tests/streaming.rs`'s `js_streams`
/// checks this directly against a live pipe rather than assuming either way.
fn fused_main(program: &Program, fusion: &tir::Fusion) -> String {
    let mut out = String::new();
    out.push_str("let tl_stdin_buf = \"\";\n");
    out.push_str("let tl_stdin_eof = false;\n");
    out.push_str("function tl_read_line() {\n");
    out.push_str("  const fs = require(\"fs\");\n");
    out.push_str("  for (;;) {\n");
    out.push_str("    const i = tl_stdin_buf.indexOf(\"\\n\");\n");
    out.push_str("    if (i !== -1) {\n");
    out.push_str("      const line = tl_stdin_buf.slice(0, i);\n");
    out.push_str("      tl_stdin_buf = tl_stdin_buf.slice(i + 1);\n");
    out.push_str("      return line;\n");
    out.push_str("    }\n");
    out.push_str("    if (tl_stdin_eof) {\n");
    out.push_str("      if (tl_stdin_buf.length === 0) return null;\n");
    out.push_str("      const line = tl_stdin_buf;\n");
    out.push_str("      tl_stdin_buf = \"\";\n");
    out.push_str("      return line;\n");
    out.push_str("    }\n");
    out.push_str("    const chunk = Buffer.alloc(65536);\n");
    out.push_str("    const n = fs.readSync(0, chunk, 0, chunk.length, null);\n");
    out.push_str("    if (n === 0) { tl_stdin_eof = true; continue; }\n");
    out.push_str("    tl_stdin_buf += chunk.toString(\"utf8\", 0, n);\n");
    out.push_str("  }\n");
    out.push_str("}\n");

    out.push_str("for (;;) {\n");
    out.push_str("  const t_line_raw = tl_read_line();\n");
    out.push_str("  if (t_line_raw === null) break;\n");
    let (mut current, mut current_ty) = match fusion.source {
        tir::Source::Inputs => {
            out.push_str("  if (t_line_raw.length === 0) continue;\n");
            out.push_str("  const t_line = JSON.parse(t_line_raw);\n");
            let elem = program
                .inputs
                .as_ref()
                .expect("an inputs source recorded its element");
            ("t_line".to_string(), elem.clone())
        }
        // A raw line is already the element, blank ones included: `lines` keeps them.
        tir::Source::Lines => ("t_line_raw".to_string(), Type::Str),
    };
    for stage in &fusion.stages {
        match stage {
            tir::Stage::Map { param, body } => {
                out.push_str(&format!("  const {} = {};\n", local(*param), current));
                current = expr(body);
                current_ty = body.ty.clone();
            }
            tir::Stage::Select { param, pred } => {
                out.push_str(&format!("  const {} = {};\n", local(*param), current));
                out.push_str(&format!("  if (!({})) continue;\n", expr(pred)));
                current = local(*param);
            }
        }
    }
    out.push_str(&format!(
        "  console.log({});\n",
        show(&current_ty, &current, 0)
    ));
    out.push_str("}\n");
    out
}

/// The printer is built from the type rather than by inspecting the value, so a record's keys
/// are known and ordered at compile time, in declaration order. That removes the whole class
/// of disagreement where one backend enumerates keys in insertion order and another sorts
/// them, and it is what a native backend will have to do anyway, having no runtime type
/// information at all.
fn show(ty: &Type, value: &str, depth: usize) -> String {
    match ty {
        Type::Param(_) => unreachable!("params are substituted before emit"),
        // The checker refuses a program whose result contains a stream, since there is nothing to
        // print: a stream has no value, only a promise that collect can redeem.
        Type::Stream(_) => unreachable!("a stream cannot reach the printer"),
        Type::Str => format!("JSON.stringify({value})"),
        Type::Int | Type::Bool => format!("String({value})"),
        Type::Vec(elem) => {
            let e = format!("e{depth}");
            format!("tl_join({value}, ({e}) => {})", show(elem, &e, depth + 1))
        }
        Type::Enum { .. } if ty.as_opt().is_some() => {
            let inner = ty.as_opt().expect("guarded");
            let v = format!("o{depth}");
            format!(
                "(({v}) => {v} === \"none\" ? \"null\" : {})({value})",
                show(inner, &format!("{v}.some"), depth + 1)
            )
        }
        // One type, two runtime shapes (ADR 0009): a unit variant is a bare string, a payload
        // variant a single-key object, so the shape is inspected before rendering. Which payload
        // follows which key is still the type's knowledge, as everywhere else.
        Type::Enum { variants, .. } => {
            let n = format!("n{depth}");
            let payloads: Vec<&(String, Option<Type>)> =
                variants.iter().filter(|(_, p)| p.is_some()).collect();
            if payloads.is_empty() {
                return format!("JSON.stringify({value})");
            }
            let mut body = String::new();
            if payloads.len() < variants.len() {
                body.push_str(&format!(
                    "typeof {n} === \"string\" ? JSON.stringify({n}) : "
                ));
            }
            for (i, (vname, pty)) in payloads.iter().enumerate() {
                let pty = pty.as_ref().expect("filtered to payload variants");
                let read = format!("{n}[{}]", js_string(vname));
                let wrapped = format!(
                    "({} + {} + \"}}\")",
                    js_string(&format!("{{\"{vname}\":")),
                    show(pty, &read, depth + 1)
                );
                if i + 1 < payloads.len() {
                    body.push_str(&format!("{read} !== undefined ? {wrapped} : "));
                } else {
                    // The last payload variant needs no test: the type says nothing else is left.
                    body.push_str(&wrapped);
                }
            }
            format!("(({n}) => {body})({value})")
        }
        Type::Record(fields) => {
            // The type's field list is declaration order, so this prints as declared. Field
            // names are identifiers, so the JSON key needs no escaping and is one literal.
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, fty)| {
                    let read = format!("{value}[{}]", js_string(name));
                    let key = js_string(&format!("\"{name}\":"));
                    format!("{key} + {}", show(fty, &read, depth + 1))
                })
                .collect();
            format!("(\"{{\" + [{}].join(\",\") + \"}}\")", parts.join(", "))
        }
    }
}

fn contains_vec(ty: &Type) -> bool {
    match ty {
        Type::Vec(_) => true,
        Type::Record(fields) => fields.iter().any(|(_, t)| contains_vec(t)),
        Type::Enum { variants, .. } => variants
            .iter()
            .any(|(_, p)| p.as_ref().is_some_and(contains_vec)),
        _ => false,
    }
}

fn user(name: &str) -> String {
    format!("v_{name}")
}

fn local(id: LocalId) -> String {
    format!("t_{id}")
}

#[derive(Default)]
struct Helpers {
    select: bool,
    field: bool,
    index: bool,
    unwrap: bool,
    arith: bool,
    collect: bool,
    jsonlines: bool,
    tail: bool,
    str_cmp: bool,
}

fn used_helpers(program: &Program) -> Helpers {
    fn walk(t: &Tir, used: &mut Helpers) {
        match &t.kind {
            Kind::Str(_)
            | Kind::Int(_)
            | Kind::Var(_)
            | Kind::Local(_)
            | Kind::Input
            | Kind::Inputs => {}
            // The source is what reads stdin now, so it is what needs the helper.
            Kind::Lines => used.collect = true,
            Kind::VecLit(items) => items.iter().for_each(|i| walk(i, used)),
            Kind::RecordLit { fields } => {
                fields.iter().for_each(|(_, v)| walk(v, used));
            }
            Kind::EnumLit { payload, .. } => {
                if let Some(p) = payload {
                    walk(p, used);
                }
            }
            Kind::Call { arg, .. } => {
                if let Some(a) = arg {
                    walk(a, used);
                }
            }
            Kind::Concat(l, r) => {
                walk(l, used);
                walk(r, used);
            }
            Kind::Compare { op, lhs: l, rhs: r } => {
                used.str_cmp |= l.ty == Type::Str
                    && matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge);
                walk(l, used);
                walk(r, used);
            }
            Kind::Bind { value, body, .. } => {
                walk(value, used);
                walk(body, used);
            }
            Kind::Map { source, body, .. } | Kind::OptMap { source, body, .. } => {
                walk(source, used);
                walk(body, used);
            }
            Kind::Select { source, pred, .. } => {
                used.select = true;
                walk(source, used);
                walk(pred, used);
            }
            Kind::Field { base, .. } => {
                used.field |= tir::vec_depth(&base.ty) > 0;
                walk(base, used);
            }
            Kind::Builtin { which, arg } => {
                used.jsonlines |= *which == Builtin::JsonLines;
                used.tail |= *which == Builtin::Tail;
                walk(arg, used);
            }
            Kind::Cond {
                cond,
                then,
                otherwise,
            } => {
                walk(cond, used);
                walk(then, used);
                walk(otherwise, used);
            }
            Kind::Arith { lhs, rhs, .. } => {
                used.arith = true;
                walk(lhs, used);
                walk(rhs, used);
            }
            Kind::Unwrap { base } => {
                used.unwrap = true;
                walk(base, used);
            }
            Kind::Index { base, index, .. } => {
                used.index = true;
                walk(base, used);
                walk(index, used);
            }
            Kind::Match { subject, arms, .. } => {
                walk(subject, used);
                for a in arms {
                    if let Some(g) = &a.guard {
                        walk(g, used);
                    }
                    walk(&a.body, used);
                }
            }
        }
    }
    let mut used = Helpers::default();
    program.funcs.iter().for_each(|f| walk(&f.body, &mut used));
    walk(&program.body, &mut used);
    used
}

/// A partial chain's yield is an Opt, so a present arm is tagged; total chains return the
/// body bare. Split out of `expr`'s match arm (kantord/toylang#62).
fn arm_return(body: String, partial: bool) -> String {
    if partial {
        format!("return {{some: {body}}}; ")
    } else {
        format!("return {body}; ")
    }
}

fn expr(t: &Tir) -> String {
    match &t.kind {
        Kind::Str(s) => js_string(s),
        Kind::Int(n) => n.to_string(),
        Kind::Var(name) => user(name),
        Kind::Local(id) => local(*id),
        Kind::Input => INPUT.to_string(),
        Kind::Inputs => INPUTS.to_string(),
        // The stream, materialized eagerly: whatever consumes it -- `collect`, a mapper --
        // works on the array of its entries. Fusion is what will remove this materialization.
        Kind::Lines => "tl_collect_lines()".to_string(),
        Kind::RecordLit { fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| format!("{}: {}", js_string(name), expr(value)))
                .collect();
            // Parenthesised because a brace opening an arrow function's body is a block, so
            // `(e) => {a: 1}` is a labelled statement rather than an object.
            format!("({{{}}})", parts.join(", "))
        }

        // The value is its JSON shape: a unit variant is the variant-name string, a payload
        // variant the single-key object a record already is.
        Kind::EnumLit { variant, payload } => match payload {
            None => js_string(variant),
            Some(p) => format!("({{{}: {}}})", js_string(variant), expr(p)),
        },

        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(expr).collect();
            format!("[{}]", parts.join(", "))
        }
        Kind::Call { func, arg } => format!(
            "{}({})",
            user(func),
            arg.as_deref().map_or_else(String::new, expr)
        ),
        Kind::Concat(l, r) => format!("({} + {})", expr(l), expr(r)),
        Kind::Arith { op, lhs, rhs } => match op {
            BinOp::Div => format!("tl_div({}, {})", expr(lhs), expr(rhs)),
            BinOp::Rem => format!("tl_rem({}, {})", expr(lhs), expr(rhs)),
            // Math.imul is the only exact 32-bit multiply here: a plain `*` loses the low bits
            // once the true product passes 2^53.
            BinOp::Mul => format!("Math.imul({}, {})", expr(lhs), expr(rhs)),
            BinOp::Add => format!("(({} + {}) | 0)", expr(lhs), expr(rhs)),
            BinOp::Sub => format!("(({} - {}) | 0)", expr(lhs), expr(rhs)),
            other => unreachable!("{other} is not arithmetic"),
        },
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => {
            format!("({} ? {} : {})", expr(cond), expr(then), expr(otherwise))
        }
        Kind::Builtin { which, arg } => match which {
            Builtin::IntToStr => format!("String({})", expr(arg)),
            Builtin::Range => {
                format!(
                    "Array.from({{ length: Math.max(0, {}) }}, (_, i) => i)",
                    expr(arg)
                )
            }
            Builtin::JsonLines => {
                let elem = tir::runtime_elem(&arg.ty).expect("checked to be a Vec or a stream");
                let e = "e0".to_string();
                format!(
                    "tl_jsonlines({}, ({e}) => {})",
                    expr(arg),
                    show(elem, &e, 1)
                )
            }
            // The source already materialized, so the exit has nothing left to do.
            Builtin::Collect => expr(arg),
            Builtin::Extent => format!("{}.length", expr(arg)),
            Builtin::Tail => format!("tl_tail({})", expr(arg)),
            Builtin::Concat => format!("{}.flat()", expr(arg)),
            // The names come from the checked type, not the object value, so `arg` is evaluated
            // only for whatever else it does (a division inside it must still throw) and its
            // value discarded with the comma operator.
            Builtin::Fields => {
                let Type::Record(fields) = &arg.ty else {
                    unreachable!("checked to be a record")
                };
                let names: Vec<String> = fields.iter().map(|(n, _)| js_string(n)).collect();
                format!("({}, [{}])", expr(arg), names.join(", "))
            }
        },
        Kind::Compare { op, lhs, rhs } => {
            // `<`/`<=`/`>`/`>=` on native JS strings compare UTF-16 code units, which disagrees
            // with codepoint order across a surrogate pair; `tl_str_cmp` steps by codepoint
            // instead. Equality is unaffected (the same codepoint sequence gives the same UTF-16
            // units either way), so `===`/`!==` stay as they are.
            if lhs.ty == Type::Str && matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) {
                format!(
                    "(tl_str_cmp({}, {}) {} 0)",
                    expr(lhs),
                    expr(rhs),
                    js_op(*op)
                )
            } else {
                format!("({} {} {})", expr(lhs), js_op(*op), expr(rhs))
            }
        }
        Kind::Bind {
            local: id,
            value,
            body,
        } => {
            format!("(({}) => {})({})", local(*id), expr(body), expr(value))
        }
        // Array.prototype.map is exactly this, so no helper is needed.
        Kind::Map {
            source,
            param,
            body,
        } => {
            format!(
                "{}.map(({}) => {})",
                expr(source),
                local(*param),
                expr(body)
            )
        }
        Kind::Select {
            source,
            param,
            pred,
        } => {
            format!(
                "tl_select({}, ({}) => {})",
                expr(source),
                local(*param),
                expr(pred)
            )
        }
        // Opt's reorder pass (kantord/toylang#66): the tagged shape (`"none"` or `{some: v}`)
        // is generic enough that this is the ordinary key-presence test every Match arm over
        // an Opt subject would already use, just rebuilding the payload instead of a body.
        Kind::OptMap {
            source,
            param,
            body,
        } => {
            format!(
                "(__opt => __opt === \"none\" ? \"none\" : (({}) => ({{some: {}}}))(__opt.some))({})",
                local(*param),
                expr(body),
                expr(source)
            )
        }
        Kind::Unwrap { base } => {
            format!("tl_unwrap({}, {})", expr(base), tir::vec_depth(&base.ty))
        }
        Kind::Index {
            base, index, depth, ..
        } => {
            format!("tl_at({}, {}, {})", expr(base), expr(index), depth)
        }
        Kind::Field { base, name } => {
            let depth = tir::vec_depth(&base.ty);
            if depth == 0 {
                format!("{}[{}]", expr(base), js_string(name))
            } else {
                format!("tl_field({}, {}, {})", expr(base), js_string(name), depth)
            }
        }
        // A chain of tests over the subject (a plain local, so re-reading it is free): string
        // equality for a unit variant, key presence for a payload variant, the guard's own
        // Bool for a guard arm. A total chain's last arm needs no test, the checker having
        // proved nothing else can reach it; a partial chain tags every present arm and falls
        // through to the absent Opt.
        Kind::Match {
            subject,
            arms,
            partial,
        } => {
            let subj = expr(subject);
            let mut body = String::new();
            for (i, arm) in arms.iter().enumerate() {
                let mut run = String::new();
                if let Some(pid) = arm.payload {
                    let variant = arm
                        .variant
                        .as_ref()
                        .expect("only a variant arm has a payload");
                    run.push_str(&format!(
                        "const {} = {subj}[{}]; ",
                        local(pid),
                        js_string(variant)
                    ));
                }
                run.push_str(&arm_return(expr(&arm.body), *partial));
                let test = match (&arm.variant, &arm.guard) {
                    (Some(v), _) if arm.payload.is_some() => {
                        Some(format!("{subj}[{}] !== undefined", js_string(v)))
                    }
                    (Some(v), _) => Some(format!("{subj} === {}", js_string(v))),
                    (None, Some(g)) => Some(expr(g)),
                    (None, None) => None,
                };
                match test {
                    Some(test) if *partial || i + 1 < arms.len() => {
                        body.push_str(&format!("if ({test}) {{ {run}}} "));
                    }
                    _ => body.push_str(&run),
                }
            }
            if *partial {
                body.push_str("return \"none\"; ");
            }
            format!("(() => {{ {body}}})()")
        }
    }
}

fn js_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "===",
        BinOp::Ne => "!==",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        other => unreachable!("{other} is not a comparison"),
    }
}

fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // U+2028 and U+2029 are line terminators in JavaScript but not in JSON, so a string
            // containing them would end the literal early if written through.
            c if (c as u32) < 0x20 || c == '\u{7f}' || c == '\u{2028}' || c == '\u{2029}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
