//! The jq backend.
//!
//! The other three targets are imperative and this one is not, so it is the only backend where
//! the compiler has to say what it means in stream terms. That makes it a check on the
//! relationship to jq rather than another way to run a program: everything the design diverges
//! on has to be bridged here explicitly.
//!
//! Two mappings are worth naming. A dimension spec becomes `.[]` plus a reification, since
//! keeping a dimension in a stream language means iterating and collecting. And `Opt` becomes
//! null, which is lossless in this direction only: toylang has no null value, so an absent entry
//! is the one thing null can mean.

use crate::ast::BinOp;
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::Type;

/// The binding stdin is read into. jq puts the input in `.`, which this backend needs for the
/// subject, so it is bound away before the program starts.
const INPUT: &str = "$t_input";

/// jq has no integer type, so 32-bit wrapping is arithmetic on doubles. Addition and
/// subtraction stay exact because the result never passes 2^33; multiplication does not, since
/// the true product needs 62 bits, so it goes through 16-bit halves whose partial products each
/// stay under 2^53. Verified against C and Math.imul, including -2147483648 * -1.
const ARITH_HELPER: &str = r#"def tl_i32: . as $r
  | ((($r % 4294967296) + 4294967296) % 4294967296)
  | if . >= 2147483648 then . - 4294967296 else . end;
def tl_mul($a; $b):
  ($a | tl_i32) as $x | ($b | tl_i32) as $y
  | (if $x < 0 then $x + 4294967296 else $x end) as $ua
  | (if $y < 0 then $y + 4294967296 else $y end) as $ub
  | ($ua / 65536 | floor) as $ah | ($ua % 65536) as $al
  | ($ub / 65536 | floor) as $bh | ($ub % 65536) as $bl
  | (($al * $bl) + ((($ah * $bl) + ($al * $bh)) % 65536) * 65536) | tl_i32;
def tl_div($a; $b):
  if $b == 0 then error("toylang: divided by zero")
  else ($a / $b | trunc) | tl_i32 end;
def tl_rem($a; $b):
  if $b == 0 then error("toylang: divided by zero") else ($a % $b) | tl_i32 end;
"#;

pub fn emit(program: &Program) -> String {
    let mut out = String::new();
    if uses_arith(program) {
        out.push_str(ARITH_HELPER);
    }

    // jq resolves a `def` only against what is already defined, so definitions have to come out
    // callee-first. The checker collects every signature before checking any body and therefore
    // accepts a call to a function defined further down, which is a rule this target does not
    // share. Lua needed forward declarations for the same reason; jq has no way to write one.
    for f in ordered(program) {
        // Functions are unary, so the argument arrives as `.` and is bound before the body runs.
        out.push_str(&format!(
            "def {}: . as ${} | {};\n",
            user(&f.name),
            user(&f.param),
            expr(&f.body)
        ));
    }

    if program.input.is_some() {
        out.push_str(&format!(". as {INPUT} | "));
    }
    // Records are rebuilt in the type's field order, because jq preserves insertion order and
    // an object read from input carries whatever order the input had.
    out.push_str(&canonical(&program.body.ty, &expr(&program.body)));
    out.push('\n');
    out
}

/// Reconstruct a value with keys in the type's order, so the printed form matches the other
/// backends rather than the input's key order.
fn canonical(ty: &Type, value: &str) -> String {
    match ty {
        Type::Str | Type::Int | Type::Bool => value.to_string(),
        Type::Vec(elem) => format!("[ {value}[] | {} ]", canonical(elem, ".")),
        Type::Opt(inner) => {
            format!("({value} | if . == null then null else {} end)", canonical(inner, "."))
        }
        Type::Record(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, fty)| {
                    format!("{}: {}", jq_string(name), canonical(fty, &field_of(".", name)))
                })
                .collect();
            format!("({value} | {{{}}})", parts.join(", "))
        }
    }
}

/// Definitions in callee-before-caller order.
///
/// Mutual recursion has no ordering that works and no forward declaration to fall back on, so it
/// is unrepresentable here rather than merely awkward. Nothing in the language can express it
/// yet, and this returns definitions unsorted rather than looping if that changes.
fn ordered(program: &Program) -> Vec<&tir::Func> {
    fn callees(t: &Tir, out: &mut Vec<String>) {
        match &t.kind {
            Kind::Str(_) | Kind::Int(_) | Kind::Var(_) | Kind::Local(_) | Kind::Input => {}
            Kind::VecLit(items) => items.iter().for_each(|i| callees(i, out)),
            Kind::Call { func, arg } => {
                out.push(func.clone());
                callees(arg, out);
            }
            Kind::Concat(l, r) | Kind::Compare { lhs: l, rhs: r, .. } => {
                callees(l, out);
                callees(r, out);
            }
            Kind::Bind { value, body, .. } => {
                callees(value, out);
                callees(body, out);
            }
            Kind::Select { source, pred, .. } => {
                callees(source, out);
                callees(pred, out);
            }
            Kind::Map { source, body, .. } => {
                callees(source, out);
                callees(body, out);
            }
            Kind::Builtin { arg, .. } => callees(arg, out),
            Kind::Cond { cond, then, otherwise } => {
                callees(cond, out);
                callees(then, out);
                callees(otherwise, out);
            }
            Kind::Arith { lhs, rhs, .. } => {
                callees(lhs, out);
                callees(rhs, out);
            }
            Kind::Field { base, .. } | Kind::Unwrap { base } => callees(base, out),
            Kind::Index { base, index, .. } => {
                callees(base, out);
                callees(index, out);
            }
        }
    }

    let mut done: Vec<&tir::Func> = Vec::new();
    let mut placed: Vec<String> = Vec::new();
    let mut remaining: Vec<&tir::Func> = program.funcs.iter().collect();

    while !remaining.is_empty() {
        let ready: Vec<usize> = remaining
            .iter()
            .enumerate()
            .filter(|(_, f)| {
                let mut calls = Vec::new();
                callees(&f.body, &mut calls);
                calls
                    .iter()
                    .all(|c| c == &f.name || placed.contains(c))
            })
            .map(|(i, _)| i)
            .collect();
        if ready.is_empty() {
            // A cycle. Emitting them in source order at least produces a jq error naming the
            // function rather than looping here.
            done.append(&mut remaining);
            break;
        }
        for i in ready.into_iter().rev() {
            let f = remaining.remove(i);
            placed.push(f.name.clone());
            done.push(f);
        }
    }
    done
}

fn uses_arith(program: &Program) -> bool {
    fn walk(t: &Tir) -> bool {
        match &t.kind {
            Kind::Arith { .. } => true,
            Kind::Cond { cond, then, otherwise } => walk(cond) || walk(then) || walk(otherwise),
            Kind::Str(_) | Kind::Int(_) | Kind::Var(_) | Kind::Local(_) | Kind::Input => false,
            Kind::VecLit(items) => items.iter().any(walk),
            Kind::Call { arg, .. } | Kind::Builtin { arg, .. } => walk(arg),
            Kind::Concat(l, r) | Kind::Compare { lhs: l, rhs: r, .. } => walk(l) || walk(r),
            Kind::Bind { value, body, .. } => walk(value) || walk(body),
            Kind::Select { source, pred, .. } => walk(source) || walk(pred),
            Kind::Map { source, body, .. } => walk(source) || walk(body),
            Kind::Field { base, .. } | Kind::Unwrap { base } => walk(base),
            Kind::Index { base, index, .. } => walk(base) || walk(index),
        }
    }
    program.funcs.iter().any(|f| walk(&f.body)) || walk(&program.body)
}

fn user(name: &str) -> String {
    format!("v_{name}")
}

fn local(id: LocalId) -> String {
    format!("$t_{id}")
}

fn field_of(base: &str, name: &str) -> String {
    format!("{base}[{}]", jq_string(name))
}

/// Apply `inner` one dimension down, `depth` times. Keeping a dimension in a stream language is
/// iterate-and-collect, which is why every level costs a `[ .[] | ... ]`.
fn distribute(inner: &str, depth: usize) -> String {
    if depth == 0 {
        inner.to_string()
    } else {
        format!("[ .[] | {} ]", distribute(inner, depth - 1))
    }
}

fn expr(t: &Tir) -> String {
    match &t.kind {
        Kind::Str(s) => jq_string(s),
        Kind::Int(n) => n.to_string(),
        Kind::Var(name) => format!("${}", user(name)),
        Kind::Local(id) => local(*id),
        Kind::Input => INPUT.to_string(),
        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(expr).collect();
            format!("[{}]", parts.join(", "))
        }
        Kind::Call { func, arg } => format!("({} | {})", expr(arg), user(func)),
        Kind::Concat(l, r) => format!("({} + {})", expr(l), expr(r)),
        Kind::Arith { op, lhs, rhs } => match op {
            BinOp::Div => format!("tl_div({}; {})", expr(lhs), expr(rhs)),
            BinOp::Rem => format!("tl_rem({}; {})", expr(lhs), expr(rhs)),
            BinOp::Mul => format!("tl_mul({}; {})", expr(lhs), expr(rhs)),
            BinOp::Add => format!("(({} + {}) | tl_i32)", expr(lhs), expr(rhs)),
            BinOp::Sub => format!("(({} - {}) | tl_i32)", expr(lhs), expr(rhs)),
            other => unreachable!("{other} is not arithmetic"),
        },
        Kind::Cond { cond, then, otherwise } => format!(
            "(if {} then {} else {} end)",
            expr(cond),
            expr(then),
            expr(otherwise)
        ),
        Kind::Builtin { which, arg } => match which {
            Builtin::IntToStr => format!("({} | tostring)", expr(arg)),
            Builtin::Range => format!("[ range(0; {}) ]", expr(arg)),
            Builtin::Unlines => format!("({} | join(\"\\n\"))", expr(arg)),
        },
        Kind::Compare { op, lhs, rhs } => {
            format!("({} {} {})", expr(lhs), jq_op(*op), expr(rhs))
        }
        Kind::Bind { local: id, value, body } => {
            format!("({} as {} | {})", expr(value), local(*id), expr(body))
        }
        // The one operator that is derived in jq and primitive here: `map(f)` is `[ .[] | f ]`
        // there, and neither half of that exists in a language with no effect layer.
        Kind::Map { source, param, body } => format!(
            "[ {}[] | . as {} | {} ]",
            expr(source),
            local(*param),
            expr(body)
        ),
        Kind::Select { source, param, pred } => format!(
            "[ {}[] | . as {} | select({}) ]",
            expr(source),
            local(*param),
            expr(pred)
        ),
        Kind::Field { base, name } => {
            let depth = tir::vec_depth(&base.ty);
            format!("({} | {})", expr(base), distribute(&field_of(".", name), depth))
        }
        Kind::Unwrap { base } => {
            let check = format!("if . == null then error({}) else . end", jq_string("toylang: unwrapped a value that is not there"));
            format!("({} | {})", expr(base), distribute(&check, tir::vec_depth(&base.ty)))
        }
        // Out of range is null in jq, which is exactly what an absent Opt is here.
        Kind::Index { base, index, depth, .. } => {
            let at = format!(".[{}]", expr(index));
            format!("({} | {})", expr(base), distribute(&at, *depth))
        }
    }
}

fn jq_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "==",
        BinOp::Ne => "!=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        other => unreachable!("{other} is not a comparison"),
    }
}

fn jq_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
