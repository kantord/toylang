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
// Absence is a sentinel: toylang has no null value, so nothing else can produce one.
const tl_none = Symbol(\"none\");
function tl_at(v, i, depth) {
  if (depth > 0) return v.map((e) => tl_at(e, i, depth - 1));
  const n = v.length;
  if (i < 0) i = n + i;
  if (i < 0 || i >= n) return tl_none;
  return v[i];
}
";

const TAIL_HELPER: &str = "\
function tl_tail(v) {
  if (v.length === 0) return tl_none;
  return v.slice(1);
}
";

const UNWRAP_HELPER: &str = r#"function tl_unwrap(v, depth) {
  if (depth > 0) return v.map((e) => tl_unwrap(e, depth - 1));
  if (v === tl_none) { throw new Error("toylang: unwrapped a value that is not there"); }
  return v;
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
    if used.index || used.unwrap || used.tail || contains_opt(&program.body.ty) {
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

    // Function declarations hoist, so a call to one defined further down resolves without the
    // forward declarations Lua needs. Each backend does what its own target does.
    for f in &program.funcs {
        out.push_str(&format!(
            "function {}({}) {{\n  return {};\n}}\n",
            user(&f.name),
            user(&f.param),
            expr(&f.body)
        ));
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
        out.push_str(&format!("console.log({});\n", show(&program.body.ty, &body, 0)));
    }
    out
}

/// The printer is built from the type rather than by inspecting the value, so a record's keys
/// are known and ordered at compile time. That removes the whole class of disagreement where
/// one backend enumerates keys in insertion order and another sorts them, and it is what a
/// native backend will have to do anyway, having no runtime type information at all.
fn show(ty: &Type, value: &str, depth: usize) -> String {
    match ty {
        // The checker refuses a program whose result contains Lines, since there is nothing to
        // print: a stream has no value, only a promise that collect can redeem.
        Type::Lines => unreachable!("Lines cannot reach the printer"),
        Type::Str => format!("JSON.stringify({value})"),
        Type::Int | Type::Bool => format!("String({value})"),
        Type::Vec(elem) => {
            let e = format!("e{depth}");
            format!("tl_join({value}, ({e}) => {})", show(elem, &e, depth + 1))
        }
        Type::Opt(inner) => {
            let v = format!("o{depth}");
            format!(
                "(({v}) => {v} === tl_none ? \"null\" : {})({value})",
                show(inner, &v, depth + 1)
            )
        }
        Type::Record(fields) => {
            // Type::record keeps fields sorted, so this order is the type's order. Field names
            // are identifiers, so the JSON key needs no escaping and is one literal.
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

fn contains_opt(ty: &Type) -> bool {
    match ty {
        Type::Opt(_) => true,
        Type::Vec(t) => contains_opt(t),
        Type::Record(fields) => fields.iter().any(|(_, t)| contains_opt(t)),
        _ => false,
    }
}

fn contains_vec(ty: &Type) -> bool {
    match ty {
        Type::Vec(_) => true,
        Type::Opt(t) => contains_vec(t),
        Type::Record(fields) => fields.iter().any(|(_, t)| contains_vec(t)),
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
}

fn used_helpers(program: &Program) -> Helpers {
    fn walk(t: &Tir, used: &mut Helpers) {
        match &t.kind {
            Kind::Str(_)
            | Kind::Int(_)
            | Kind::Var(_)
            | Kind::Local(_)
            | Kind::Input
            | Kind::Inputs
            | Kind::Lines => {}
            Kind::VecLit(items) => items.iter().for_each(|i| walk(i, used)),
            Kind::RecordLit { fields } => {
                fields.iter().for_each(|(_, v)| walk(v, used));
            }
            Kind::Call { arg, .. } => walk(arg, used),
            Kind::Concat(l, r) | Kind::Compare { lhs: l, rhs: r, .. } => {
                walk(l, used);
                walk(r, used);
            }
            Kind::Bind { value, body, .. } => {
                walk(value, used);
                walk(body, used);
            }
            Kind::Map { source, body, .. } => {
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
                used.collect |= *which == Builtin::Collect;
                used.jsonlines |= *which == Builtin::JsonLines;
                used.tail |= *which == Builtin::Tail;
                walk(arg, used);
            }
            Kind::Cond { cond, then, otherwise } => {
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
        }
    }
    let mut used = Helpers::default();
    program.funcs.iter().for_each(|f| walk(&f.body, &mut used));
    walk(&program.body, &mut used);
    used
}

fn expr(t: &Tir) -> String {
    match &t.kind {
        Kind::Str(s) => js_string(s),
        Kind::Int(n) => n.to_string(),
        Kind::Var(name) => user(name),
        Kind::Local(id) => local(*id),
        Kind::Input => INPUT.to_string(),
        Kind::Inputs => INPUTS.to_string(),
        // `lines` has no value of its own -- it is a promise that the real stdin has not been
        // read yet, made good only by `collect`. `undefined` is never actually inspected.
        Kind::Lines => "undefined".to_string(),
        Kind::RecordLit { fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| format!("{}: {}", js_string(name), expr(value)))
                .collect();
            // Parenthesised because a brace opening an arrow function's body is a block, so
            // `(e) => {a: 1}` is a labelled statement rather than an object.
            format!("({{{}}})", parts.join(", "))
        }

        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(expr).collect();
            format!("[{}]", parts.join(", "))
        }
        Kind::Call { func, arg } => format!("{}({})", user(func), expr(arg)),
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
        Kind::Cond { cond, then, otherwise } => {
            format!("({} ? {} : {})", expr(cond), expr(then), expr(otherwise))
        }
        Kind::Builtin { which, arg } => match which {
            Builtin::IntToStr => format!("String({})", expr(arg)),
            Builtin::Range => {
                format!("Array.from({{ length: Math.max(0, {}) }}, (_, i) => i)", expr(arg))
            }
            Builtin::JsonLines => {
                let elem = arg.ty.elem().expect("checked to be a Vec");
                let e = "e0".to_string();
                format!("tl_jsonlines({}, ({e}) => {})", expr(arg), show(elem, &e, 1))
            }
            Builtin::Collect => "tl_collect_lines()".to_string(),
            Builtin::Extent => format!("{}.length", expr(arg)),
            Builtin::Tail => format!("tl_tail({})", expr(arg)),
            Builtin::Concat => format!("{}.flat()", expr(arg)),
        },
        Kind::Compare { op, lhs, rhs } => {
            format!("({} {} {})", expr(lhs), js_op(*op), expr(rhs))
        }
        Kind::Bind { local: id, value, body } => {
            format!("(({}) => {})({})", local(*id), expr(body), expr(value))
        }
        // Array.prototype.map is exactly this, so no helper is needed.
        Kind::Map { source, param, body } => {
            format!("{}.map(({}) => {})", expr(source), local(*param), expr(body))
        }
        Kind::Select { source, param, pred } => {
            format!("tl_select({}, ({}) => {})", expr(source), local(*param), expr(pred))
        }
        Kind::Unwrap { base } => {
            format!("tl_unwrap({}, {})", expr(base), tir::vec_depth(&base.ty))
        }
        Kind::Index { base, index, depth, .. } => {
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
