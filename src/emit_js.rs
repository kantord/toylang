use crate::ast::BinOp;
use crate::tir::{self, Kind, LocalId, Program, Tir};
use crate::ty::Type;

/// The binding the input value is read into. Unspellable in source, since every source name is
/// prefixed.
const INPUT: &str = "t_input";

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

const JOIN_HELPER: &str = "\
function tl_join(v, f) {
  const parts = [];
  for (let i = 0; i < v.length; i++) parts.push(f(v[i]));
  return \"[\" + parts.join(\",\") + \"]\";
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
    if matches!(program.body.ty, Type::Vec(_)) || contains_vec(&program.body.ty) {
        out.push_str(JOIN_HELPER);
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
        Type::Str => format!("JSON.stringify({value})"),
        Type::Int | Type::Bool => format!("String({value})"),
        Type::Vec(elem) => {
            let e = format!("e{depth}");
            format!("tl_join({value}, ({e}) => {})", show(elem, &e, depth + 1))
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

fn contains_vec(ty: &Type) -> bool {
    match ty {
        Type::Vec(_) => true,
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
}

fn used_helpers(program: &Program) -> Helpers {
    fn walk(t: &Tir, used: &mut Helpers) {
        match &t.kind {
            Kind::Str(_) | Kind::Int(_) | Kind::Var(_) | Kind::Local(_) | Kind::Input => {}
            Kind::VecLit(items) => items.iter().for_each(|i| walk(i, used)),
            Kind::Call { arg, .. } => walk(arg, used),
            Kind::Concat(l, r) | Kind::Compare { lhs: l, rhs: r, .. } => {
                walk(l, used);
                walk(r, used);
            }
            Kind::Bind { value, body, .. } => {
                walk(value, used);
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
        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(expr).collect();
            format!("[{}]", parts.join(", "))
        }
        Kind::Call { func, arg } => format!("{}({})", user(func), expr(arg)),
        Kind::Concat(l, r) => format!("({} + {})", expr(l), expr(r)),
        Kind::Compare { op, lhs, rhs } => {
            format!("({} {} {})", expr(lhs), js_op(*op), expr(rhs))
        }
        Kind::Bind { local: id, value, body } => {
            format!("(({}) => {})({})", local(*id), expr(body), expr(value))
        }
        Kind::Select { source, param, pred } => {
            format!("tl_select({}, ({}) => {})", expr(source), local(*param), expr(pred))
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
        BinOp::Add => unreachable!("Add is emitted as Concat"),
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
