use crate::ast::{BinOp, LogicOp};
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::{self, Enums, Type};

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

// `Array.prototype.slice` already clamps out-of-range bounds and counts negatives from the
// end, so jq's boundary behaviour is the target's native one; `undefined` is a bound left
// out, which `.slice` reads as the array's own boundary.
const SLICE_HELPER: &str = "\
function tl_slice(v, lo, hi, depth) {
  if (depth > 0) return v.map((e) => tl_slice(e, lo, hi, depth - 1));
  return v.slice(lo, hi);
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

// An Int64 is a BigInt (kantord/toylang#83): a double is off past 2^53, and BigInt's own `/`
// and `%` already truncate with the dividend's sign. `BigInt.asIntN(64, ...)` is the wrap,
// including `MIN / -1` back to `MIN`.
const ARITH64_HELPER: &str = r#"function tl_div64(a, b) {
  if (b === 0n) { throw new Error("toylang: divided by zero"); }
  return BigInt.asIntN(64, a / b);
}
function tl_rem64(a, b) {
  if (b === 0n) { throw new Error("toylang: divided by zero"); }
  return BigInt.asIntN(64, a % b);
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

// A record and a payload-carrying variant are both objects, and JS `===` on two objects is
// identity, so `{a: 1} === {a: 1}` was false here and true on Python (kantord/toylang#68).
// Structural equality is the ratified answer, so composites walk. Keyed rather than ordered:
// two spellings of one record type may reach here with their keys in different orders.
const EQ_HELPER: &str = "\
function tl_eq(a, b) {
  if (a === b) return true;
  if (typeof a !== \"object\" || a === null || typeof b !== \"object\" || b === null) return false;
  const keys = Object.keys(a);
  if (keys.length !== Object.keys(b).length) return false;
  return keys.every((k) => Object.hasOwn(b, k) && tl_eq(a[k], b[k]));
}
";

const JSONLINES_HELPER: &str = "\
function tl_jsonlines(v, f) {
  const parts = [];
  for (let i = 0; i < v.length; i++) parts.push(f(v[i]));
  return parts.join(\"\\n\");
}
";

// The string iterator steps one codepoint at a time, surrogate pairs included (the same
// iterator STR_CMP_HELPER uses above), so there is no decoding to get right here.
const CHARS_HELPER: &str = "\
function tl_chars(s) {
  return Array.from(s, (c) => c.codePointAt(0));
}
";

// Int and Int64 are different runtime types here -- Number and BigInt -- so `+` cannot mix
// them and each width needs its own fold. `|0` is the 32-bit wrap (the same ToInt32 ARITH_HELPER
// relies on); `BigInt.asIntN(64, ...)` is the 64-bit one ARITH64_HELPER uses.
const SUM_HELPER: &str = "\
function tl_sum(v) {
  let acc = 0;
  for (let i = 0; i < v.length; i++) acc = (acc + v[i]) | 0;
  return acc;
}
function tl_sum64(v) {
  let acc = 0n;
  for (let i = 0; i < v.length; i++) acc = BigInt.asIntN(64, acc + v[i]);
  return acc;
}
";

// `>` orders Number and BigInt alike, so one maximum serves both widths.
const MAX_HELPER: &str = "\
function tl_max(v) {
  if (v.length === 0) return \"none\";
  let m = v[0];
  for (let i = 1; i < v.length; i++) if (v[i] > m) m = v[i];
  return { some: m };
}
";

pub fn emit(program: &Program) -> String {
    let enums = &program.enums;
    let mut out = String::new();

    let used = used_helpers(program);
    let join = matches!(program.body.ty, Type::Vec(_))
        || contains_vec(enums, &program.body.ty)
        || used.jsonlines;
    for (on, text) in [
        (used.select, SELECT_HELPER),
        (used.field, FIELD_HELPER),
        (join, JOIN_HELPER),
        (used.index, OPT_HELPER),
        (used.slice, SLICE_HELPER),
        (used.tail, TAIL_HELPER),
        (used.unwrap, UNWRAP_HELPER),
        (used.arith, ARITH_HELPER),
        (used.arith64, ARITH64_HELPER),
        (used.collect, COLLECT_HELPER),
        (used.jsonlines, JSONLINES_HELPER),
        (used.str_cmp, STR_CMP_HELPER),
        (used.chars, CHARS_HELPER),
        (used.sum || used.sum64, SUM_HELPER),
        (used.max, MAX_HELPER),
        (used.eq, EQ_HELPER),
    ] {
        if on {
            out.push_str(text);
        }
    }
    out.push_str(&printers(program));

    // Function declarations hoist, so a call to one defined further down resolves without the
    // forward declarations Lua needs. Each backend does what its own target does.
    for f in &program.funcs {
        let param = f.param.as_deref().map(user);
        if tir::has_tail_call(&f.name, &f.body) {
            // The contract: a self-tail-call runs in constant stack. A tail call becomes
            // `param = arg; continue;` in a loop, so 100k-deep self-recursion cannot blow the
            // JS stack the way a real call would (kantord/toylang#141).
            let mut fresh = 0;
            out.push_str(&format!("function {}({}) {{\n", user(&f.name), param.as_deref().unwrap_or_default()));
            out.push_str("  while (true) {\n");
            out.push_str(&indent(&indent(&tail_stmts(
                enums, &f.name, param.as_deref(), &mut fresh, &f.body
            ))));
            out.push_str("  }\n}\n");
        } else {
            out.push_str(&format!(
                "function {}({}) {{\n  return {};\n}}\n",
                user(&f.name),
                param.as_deref().unwrap_or_default(),
                expr(enums, &f.body)
            ));
        }
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

    let body = expr(enums, &program.body);
    // A top-level Str prints raw, the way jq's -r does; anything else prints as JSON.
    if matches!(program.body.ty, Type::Str | Type::Sink) {
        out.push_str(&format!("console.log({body});\n"));
    } else {
        out.push_str(&format!(
            "console.log({});\n",
            show(enums, &program.body.ty, &body, 0)
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
    let enums = &program.enums;
    let mut out = String::new();
    let (mut current, mut current_ty) = match fusion.source {
        tir::Source::Inputs => {
            out.push_str(&read_line_helper());
            out.push_str("for (;;) {\n");
            out.push_str("  const t_line_raw = tl_read_line();\n");
            out.push_str("  if (t_line_raw === null) break;\n");
            out.push_str("  if (t_line_raw.length === 0) continue;\n");
            out.push_str("  const t_line = JSON.parse(t_line_raw);\n");
            let elem = program
                .inputs
                .as_ref()
                .expect("an inputs source recorded its element");
            ("t_line".to_string(), elem.clone())
        }
        // A raw line is already the element, blank ones included: `lines` keeps them.
        tir::Source::Lines => {
            out.push_str(&read_line_helper());
            out.push_str("for (;;) {\n");
            out.push_str("  const t_line_raw = tl_read_line();\n");
            out.push_str("  if (t_line_raw === null) break;\n");
            ("t_line_raw".to_string(), Type::Str)
        }
        // The bound is evaluated once; the loop counter is the element. A negative bound makes
        // the loop body never run, the same answer `tl_range` gives eagerly.
        tir::Source::Range(bound) => {
            out.push_str(&format!("const n = {};\n", expr(enums, bound)));
            out.push_str("for (let t_i = 0; t_i < n; t_i++) {\n");
            ("t_i".to_string(), Type::Int)
        }
    };
    for stage in &fusion.stages {
        match stage {
            tir::Stage::Map { param, body } => {
                out.push_str(&format!("  const {} = {};\n", local(*param), current));
                current = expr(enums, body);
                current_ty = body.ty.clone();
            }
            tir::Stage::Select { param, pred } => {
                out.push_str(&format!("  const {} = {};\n", local(*param), current));
                out.push_str(&format!("  if (!({})) continue;\n", expr(enums, pred)));
                current = local(*param);
            }
        }
    }
    out.push_str(&format!(
        "  console.log({});\n",
        show(enums, &current_ty, &current, 0)
    ));
    out.push_str("}\n");
    out
}

/// The read-one-line-at-a-time machinery a stdin-backed fused loop needs. A `range` source reads
/// nothing, so it never emits this.
fn read_line_helper() -> String {
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
    out
}

/// The printer is built from the type rather than by inspecting the value, so a record's keys
/// are known and ordered at compile time, in declaration order. That removes the whole class
/// of disagreement where one backend enumerates keys in insertion order and another sorts
/// them, and it is what a native backend will have to do anyway, having no runtime type
/// information at all.
fn show(enums: &Enums, ty: &Type, value: &str, depth: usize) -> String {
    match ty {
        Type::Param(_) => unreachable!("params are substituted before emit"),
        // The checker refuses a program whose result contains a stream, since there is nothing to
        // print: a stream has no value, only a promise that collect can redeem.
        Type::Stream(_) => unreachable!("a stream cannot reach the printer"),
        Type::Char => unreachable!("Char cannot reach the printer, refused by the checker"),
        Type::Str => format!("JSON.stringify({value})"),
        Type::Sink => unreachable!("a sink only ever prints raw, never through the printer"),
        // String() on a BigInt is the bare digits, no `n` suffix, so Int64 rides the same arm.
        Type::Int | Type::Int64 | Type::Bool => format!("String({value})"),
        Type::Vec(elem) => {
            let e = format!("e{depth}");
            format!(
                "tl_join({value}, ({e}) => {})",
                show(enums, elem, &e, depth + 1)
            )
        }
        Type::Enum { .. } if ty.as_opt().is_some() => {
            let inner = ty.as_opt().expect("guarded");
            let v = format!("o{depth}");
            format!(
                "(({v}) => {v} === \"none\" ? \"null\" : {})({value})",
                show(enums, inner, &format!("{v}.some"), depth + 1)
            )
        }
        // A recursive enum prints through a function of its own (`printers`), because expanding
        // one here has no bottom: its payload leads back to the same type.
        Type::Enum { .. } if ty::is_recursive(enums, ty) => format!("{}({value})", ty.show_fn()),
        Type::Enum { .. } => show_enum(enums, ty, value, depth),
        Type::Record(fields) => {
            // The type's field list is declaration order, so this prints as declared. Field
            // names are identifiers, so the JSON key needs no escaping and is one literal.
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, fty)| {
                    let read = format!("{value}[{}]", js_string(name));
                    let key = js_string(&format!("\"{name}\":"));
                    format!("{key} + {}", show(enums, fty, &read, depth + 1))
                })
                .collect();
            format!("(\"{{\" + [{}].join(\",\") + \"}}\")", parts.join(", "))
        }
    }
}

/// The printer for one enum, inline. One type, two runtime shapes (ADR 0009): a unit variant is
/// a bare string, a payload variant a single-key object, so the shape is inspected before
/// rendering. Which payload follows which key is still the type's knowledge, as everywhere else.
fn show_enum(enums: &Enums, ty: &Type, value: &str, depth: usize) -> String {
    let variants = ty::variants(enums, ty);
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
            show(enums, pty, &read, depth + 1)
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

/// A named printer for every recursive enum the program prints. The call in `show` above is
/// what a nested occurrence renders as, so the recursion in the type becomes recursion in the
/// emitted function rather than in this compiler (kantord/toylang#94).
fn printers(program: &Program) -> String {
    let mut out = String::new();
    for ty in tir::printed_recursive_enums(program) {
        out.push_str(&format!(
            "function {}(v) {{\n  return {};\n}}\n",
            ty.show_fn(),
            show_enum(&program.enums, &ty, "v", 0)
        ));
    }
    out
}

fn contains_vec(enums: &Enums, ty: &Type) -> bool {
    match ty {
        Type::Vec(_) => true,
        Type::Record(fields) => fields.iter().any(|(_, t)| contains_vec(enums, t)),
        // The Vec arm answers before this one can loop: a self-reference is legal only behind
        // a Vec, so a recursive enum's first payload hop is one.
        Type::Enum { .. } => ty::variants(enums, ty)
            .iter()
            .any(|(_, p)| p.as_ref().is_some_and(|p| contains_vec(enums, p))),
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
    slice: bool,
    unwrap: bool,
    arith: bool,
    arith64: bool,
    collect: bool,
    jsonlines: bool,
    tail: bool,
    str_cmp: bool,
    chars: bool,
    sum: bool,
    sum64: bool,
    max: bool,
    eq: bool,
}

/// Which helper a comparison reaches for, decided by the operator and the operand type rather
/// than by anything in the tree below it: ordering on a Str has to step by codepoint, and
/// equality on a composite has to walk the structure.
fn compare_helpers(op: BinOp, operand: &Type, used: &mut Helpers) {
    used.str_cmp |=
        *operand == Type::Str && matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge);
    used.eq |= operand.is_composite() && matches!(op, BinOp::Eq | BinOp::Ne);
}

/// Two of the six comparison operators cannot be handed to JS as they are written.
/// `<`/`<=`/`>`/`>=` on native strings compare UTF-16 code units, which disagrees with codepoint
/// order across a surrogate pair, so `tl_str_cmp` steps by codepoint instead; equality is
/// unaffected there (the same codepoints give the same units either way). `===` on two objects
/// is identity, so equality on a composite walks the structure through `tl_eq`
/// (kantord/toylang#68). Ordering on a composite is a separate open question and keeps whatever
/// JS does with it.
fn compare(enums: &Enums, op: BinOp, lhs: &Tir, rhs: &Tir) -> String {
    if lhs.ty.is_composite() && matches!(op, BinOp::Eq | BinOp::Ne) {
        let call = format!("tl_eq({}, {})", expr(enums, lhs), expr(enums, rhs));
        return match op {
            BinOp::Ne => format!("(!{call})"),
            _ => call,
        };
    }
    if lhs.ty == Type::Str && matches!(op, BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge) {
        return format!(
            "(tl_str_cmp({}, {}) {} 0)",
            expr(enums, lhs),
            expr(enums, rhs),
            js_op(op)
        );
    }
    format!("({} {} {})", expr(enums, lhs), js_op(op), expr(enums, rhs))
}

/// `+` on two arrays stringifies and joins them, so a Vec reaches for `.concat` instead.
fn concat(enums: &Enums, ty: &Type, l: &Tir, r: &Tir) -> String {
    match ty {
        Type::Vec(_) => format!("{}.concat({})", expr(enums, l), expr(enums, r)),
        _ => format!("({} + {})", expr(enums, l), expr(enums, r)),
    }
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
            // A variant's payload and a call's argument are the same shape here -- one optional
            // child, recursed into and nothing else -- so they share the arm.
            Kind::EnumLit { payload: child, .. } | Kind::Call { arg: child, .. } => {
                if let Some(c) = child {
                    walk(c, used);
                }
            }
            Kind::Concat(l, r) | Kind::Logic { lhs: l, rhs: r, .. } => {
                walk(l, used);
                walk(r, used);
            }
            Kind::Compare { op, lhs: l, rhs: r } => {
                compare_helpers(*op, &l.ty, used);
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
                used.chars |= *which == Builtin::Chars;
                used.str_cmp |=
                    *which == Builtin::Sort && tir::runtime_elem(&arg.ty) == Some(&Type::Str);
                used.sum |= *which == Builtin::Sum && tir::runtime_elem(&arg.ty) == Some(&Type::Int);
                used.sum64 |=
                    *which == Builtin::Sum && tir::runtime_elem(&arg.ty) == Some(&Type::Int64);
                used.max |= *which == Builtin::Max;
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
                if t.ty == Type::Int64 {
                    used.arith64 = true;
                } else {
                    used.arith = true;
                }
                walk(lhs, used);
                walk(rhs, used);
            }
            Kind::Not(base) => walk(base, used),
            Kind::Unwrap { base } => {
                used.unwrap = true;
                walk(base, used);
            }
            Kind::Index { base, index, .. } => {
                used.index = true;
                walk(base, used);
                walk(index, used);
            }
            Kind::Slice { base, start, end, .. } => {
                used.slice = true;
                walk(base, used);
                if let Some(s) = start {
                    walk(s, used);
                }
                if let Some(e) = end {
                    walk(e, used);
                }
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

/// `t` as statements in the tail position: `return <expr>;` for a base case, and for a tail
/// call `param = <arg>; continue;` so the loop in the emitted function rewinds instead of
/// growing the JS stack. Lines come out unindented; `indent` places them under the `while`.
fn tail_stmts(
    enums: &Enums,
    name: &str,
    param: Option<&str>,
    fresh: &mut usize,
    t: &Tir,
) -> String {
    match &t.kind {
        Kind::Call { func, arg } if func == name => {
            let assign = param.map_or_else(String::new, |p| {
                format!(
                    "{p} = {};\n",
                    arg.as_deref().map_or_else(String::new, |a| expr(enums, a))
                )
            });
            format!("{assign}continue;\n")
        }
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => format!(
            "if ({}) {{\n{}}} else {{\n{}}}\n",
            expr(enums, cond),
            indent(&tail_stmts(enums, name, param, fresh, then)),
            indent(&tail_stmts(enums, name, param, fresh, otherwise)),
        ),
        Kind::Bind {
            local: id,
            value,
            body,
        } => format!(
            "const {} = {};\n{}",
            local(*id),
            expr(enums, value),
            tail_stmts(enums, name, param, fresh, body)
        ),
        Kind::Match {
            subject,
            arms,
            partial,
        } if !partial => {
            // The subject is read into a temp the way `expr`'s IIFE does, but there is no IIFE
            // to contain it here, so the name has to be fresh rather than the fixed `subj`.
            let subj = format!("tl_tailsub{}", *fresh);
            *fresh += 1;
            let mut out = format!("const {subj} = {};\n", expr(enums, subject));
            for (i, arm) in arms.iter().enumerate() {
                let mut run = String::new();
                if let Some(pid) = arm.payload {
                    let variant = arm
                        .variant
                        .as_ref()
                        .expect("only a variant arm has a payload");
                    run.push_str(&format!(
                        "const {} = {subj}[{}];\n",
                        local(pid),
                        js_string(variant)
                    ));
                }
                run.push_str(&tail_stmts(enums, name, param, fresh, &arm.body));
                let test = match (&arm.variant, &arm.guard) {
                    (Some(v), _) if arm.payload.is_some() => {
                        Some(format!("{subj}[{}] !== undefined", js_string(v)))
                    }
                    (Some(v), _) => Some(format!("{subj} === {}", js_string(v))),
                    (None, Some(g)) => Some(expr(enums, g)),
                    (None, None) => None,
                };
                match test {
                    // A total chain's last arm needs no test, the checker having proved nothing
                    // else can reach it -- the same rule `expr`'s match arm follows.
                    Some(test) if i + 1 < arms.len() => {
                        out.push_str(&format!("if ({test}) {{\n{}}}\n", indent(&run)));
                    }
                    _ => out.push_str(&run),
                }
            }
            out
        }
        // A partial match wraps every arm body, so no arm body is a tail position; the whole
        // match stays an expression, exactly as it would outside tail position.
        Kind::Match { .. } => format!("return {};\n", expr(enums, t)),
        _ => format!("return {};\n", expr(enums, t)),
    }
}

/// Prepend two spaces to every line. `tail_stmts` returns its lines unindented, and each
/// nesting level (`while` body, an `if` branch) shifts by one indent.
fn indent(s: &str) -> String {
    let mut out = String::new();
    for line in s.trim_end_matches('\n').split('\n') {
        out.push_str("  ");
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn expr(enums: &Enums, t: &Tir) -> String {
    match &t.kind {
        Kind::Str(s) => js_string(s),
        Kind::Int(n) => int_lit(&t.ty, *n),
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
                .map(|(name, value)| format!("{}: {}", js_string(name), expr(enums, value)))
                .collect();
            // Parenthesised because a brace opening an arrow function's body is a block, so
            // `(e) => {a: 1}` is a labelled statement rather than an object.
            format!("({{{}}})", parts.join(", "))
        }

        // The value is its JSON shape: a unit variant is the variant-name string, a payload
        // variant the single-key object a record already is.
        Kind::EnumLit { variant, payload } => match payload {
            None => js_string(variant),
            Some(p) => format!("({{{}: {}}})", js_string(variant), expr(enums, p)),
        },

        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(|i| expr(enums, i)).collect();
            format!("[{}]", parts.join(", "))
        }
        Kind::Call { func, arg } => format!(
            "{}({})",
            user(func),
            arg.as_deref().map_or_else(String::new, |a| expr(enums, a))
        ),
        Kind::Concat(l, r) => concat(enums, &t.ty, l, r),
        Kind::Arith { op, lhs, rhs } => arith(&t.ty, *op, expr(enums, lhs), expr(enums, rhs)),
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => {
            format!(
                "({} ? {} : {})",
                expr(enums, cond),
                expr(enums, then),
                expr(enums, otherwise)
            )
        }
        Kind::Logic { op, lhs, rhs } => {
            let op = match op {
                LogicOp::And => "&&",
                LogicOp::Or => "||",
            };
            format!("({} {op} {})", expr(enums, lhs), expr(enums, rhs))
        }
        Kind::Not(base) => format!("(!{})", expr(enums, base)),
        Kind::Builtin { which, arg } => match which {
            Builtin::IntToStr => format!("String({})", expr(enums, arg)),
            // The one real conversion among the backends: an Int is a number and an Int64 is
            // a BigInt, and BigInt() of a 32-bit integer is always exact.
            Builtin::IntToI64 => format!("BigInt({})", expr(enums, arg)),
            Builtin::Chars => format!("tl_chars({})", expr(enums, arg)),
            Builtin::Range => {
                format!(
                    "Array.from({{ length: Math.max(0, {}) }}, (_, i) => i)",
                    expr(enums, arg)
                )
            }
            Builtin::JsonLines => {
                let elem = tir::runtime_elem(&arg.ty).expect("checked to be a Vec or a stream");
                let e = "e0".to_string();
                format!(
                    "tl_jsonlines({}, ({e}) => {})",
                    expr(enums, arg),
                    show(enums, elem, &e, 1)
                )
            }
            // The source already materialized, so the exit has nothing left to do.
            Builtin::Collect => expr(enums, arg),
            Builtin::Length => format!("{}.length", expr(enums, arg)),
            Builtin::Tail => format!("tl_tail({})", expr(enums, arg)),
            Builtin::Flatten => format!("{}.flat()", expr(enums, arg)),
            // `Array.prototype.sort`'s default comparator stringifies, which is wrong for
            // numbers; `tl_str_cmp` already returns the -1/0/1 a comparator wants, so it can be
            // passed straight through for Str.
            Builtin::Sort => {
                let elem = tir::runtime_elem(&arg.ty).expect("checked to be a Vec");
                if *elem == Type::Str {
                    format!("[...{}].sort(tl_str_cmp)", expr(enums, arg))
                } else {
                    // `a - b` would coerce the comparator's result with ToNumber, which
                    // throws on BigInt -- and Int64 is BigInt here. The three-way compare
                    // returns plain -1/0/1 for Number and BigInt alike.
                    format!(
                        "[...{}].sort((a, b) => a < b ? -1 : a > b ? 1 : 0)",
                        expr(enums, arg)
                    )
                }
            }
            // `.reverse()` mutates in place, so the spread copy is what keeps this an ordinary
            // expression rather than a statement with a visible side effect on `arg`.
            Builtin::Reverse => format!("[...{}].reverse()", expr(enums, arg)),
            // One fold per width, since the element type picks the runtime integer type.
            Builtin::Sum => {
                if tir::runtime_elem(&arg.ty) == Some(&Type::Int) {
                    format!("tl_sum({})", expr(enums, arg))
                } else {
                    format!("tl_sum64({})", expr(enums, arg))
                }
            }
            Builtin::Max => format!("tl_max({})", expr(enums, arg)),
            // The names come from the checked type, not the object value, so `arg` is evaluated
            // only for whatever else it does (a division inside it must still throw) and its
            // value discarded with the comma operator.
            Builtin::Fields => {
                let Type::Record(fields) = &arg.ty else {
                    unreachable!("checked to be a record")
                };
                let names: Vec<String> = fields.iter().map(|(n, _)| js_string(n)).collect();
                format!("({}, [{}])", expr(enums, arg), names.join(", "))
            }
        },
        Kind::Compare { op, lhs, rhs } => compare(enums, *op, lhs, rhs),
        Kind::Bind {
            local: id,
            value,
            body,
        } => {
            format!(
                "(({}) => {})({})",
                local(*id),
                expr(enums, body),
                expr(enums, value)
            )
        }
        // Array.prototype.map is exactly this, so no helper is needed.
        Kind::Map {
            source,
            param,
            body,
        } => {
            format!(
                "{}.map(({}) => {})",
                expr(enums, source),
                local(*param),
                expr(enums, body)
            )
        }
        Kind::Select {
            source,
            param,
            pred,
        } => {
            format!(
                "tl_select({}, ({}) => {})",
                expr(enums, source),
                local(*param),
                expr(enums, pred)
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
                expr(enums, body),
                expr(enums, source)
            )
        }
        Kind::Unwrap { base } => {
            format!(
                "tl_unwrap({}, {})",
                expr(enums, base),
                tir::vec_depth(&base.ty)
            )
        }
        Kind::Index {
            base, index, depth, ..
        } => {
            format!(
                "tl_at({}, {}, {})",
                expr(enums, base),
                expr(enums, index),
                depth
            )
        }
        Kind::Slice {
            base, start, end, depth,
        } => {
            let lo = match start {
                Some(s) => expr(enums, s),
                None => "undefined".to_string(),
            };
            let hi = match end {
                Some(e) => expr(enums, e),
                None => "undefined".to_string(),
            };
            format!(
                "tl_slice({}, {}, {}, {})",
                expr(enums, base),
                lo,
                hi,
                depth
            )
        }
        Kind::Field { base, name } => {
            let depth = tir::vec_depth(&base.ty);
            if depth == 0 {
                format!("{}[{}]", expr(enums, base), js_string(name))
            } else {
                format!(
                    "tl_field({}, {}, {})",
                    expr(enums, base),
                    js_string(name),
                    depth
                )
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
            let subj = expr(enums, subject);
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
                run.push_str(&arm_return(expr(enums, &arm.body), *partial));
                let test = match (&arm.variant, &arm.guard) {
                    (Some(v), _) if arm.payload.is_some() => {
                        Some(format!("{subj}[{}] !== undefined", js_string(v)))
                    }
                    (Some(v), _) => Some(format!("{subj} === {}", js_string(v))),
                    (None, Some(g)) => Some(expr(enums, g)),
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

/// The node's type picks the literal's representation (kantord/toylang#83): an Int64 literal
/// is a BigInt literal, exact at every written value where a bare number would already have
/// rounded past 2^53.
fn int_lit(ty: &Type, n: i64) -> String {
    if *ty == Type::Int64 {
        format!("{n}n")
    } else {
        n.to_string()
    }
}

/// One arithmetic expression at the width the node's type names. BigInt operators are exact
/// at any width, so on the 64-bit side only the wrap needs spelling -- Math.imul has no
/// 64-bit sibling and none is needed.
fn arith(ty: &Type, op: BinOp, l: String, r: String) -> String {
    if *ty == Type::Int64 {
        match op {
            BinOp::Div => format!("tl_div64({l}, {r})"),
            BinOp::Rem => format!("tl_rem64({l}, {r})"),
            BinOp::Mul => format!("BigInt.asIntN(64, {l} * {r})"),
            BinOp::Add => format!("BigInt.asIntN(64, {l} + {r})"),
            BinOp::Sub => format!("BigInt.asIntN(64, {l} - {r})"),
            other => unreachable!("{other} is not arithmetic"),
        }
    } else {
        match op {
            BinOp::Div => format!("tl_div({l}, {r})"),
            BinOp::Rem => format!("tl_rem({l}, {r})"),
            // Math.imul is the only exact 32-bit multiply here: a plain `*` loses the low
            // bits once the true product passes 2^53.
            BinOp::Mul => format!("Math.imul({l}, {r})"),
            BinOp::Add => format!("(({l} + {r}) | 0)"),
            BinOp::Sub => format!("(({l} - {r}) | 0)"),
            other => unreachable!("{other} is not arithmetic"),
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
