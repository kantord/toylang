//! The Python backend.
//!
//! Python is dynamically typed, so the depth-polymorphic helpers that Go could not have are
//! available again: one `tl_field(v, k, depth)` serves every shape. What it stresses instead is
//! arithmetic, from a direction none of the other five come from. Its integers are exact and
//! unbounded, so wrapping is emulation like jq's -- but unlike jq nothing is lost on the way,
//! so the whole rule is one modulo rather than a split into 16-bit halves. Its `//` and `%` are
//! floored, so truncated division needs writing out.
//!
//! One mapping is better here than anywhere else: a record is a dict, which is also what
//! `json.loads` produces, so reading input is the parse and nothing more. Go needed a declared
//! struct and a decoder to reach the same place.

use crate::ast::BinOp;
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::{self, Enums, Type};

/// The binding the input value is read into. Unspellable in source, since every source name is
/// prefixed.
const INPUT: &str = "t_input";
const INPUTS: &str = "t_inputs";

const FAIL_HELPER: &str = r#"def tl_fail(msg):
    sys.stderr.write("toylang: " + msg + "\n")
    sys.exit(1)
"#;

/// Python's integers do not overflow, so the 32-bit rule is entirely emulated. It is exact
/// arithmetic all the way through, which is what lets one modulo stand for the whole thing:
/// jq needed a 16-bit split only because a double loses the low bits of a 62-bit product.
const I32_HELPER: &str = r#"def tl_i32(n):
    return ((n + 2147483648) % 4294967296) - 2147483648
"#;

/// The same one-modulo emulation at 64 bits, for `Int64` (kantord/toylang#83). Exact for the
/// same reason `tl_i32` is: Python's integers never lose bits on the way to the modulo.
const I64_HELPER: &str = r#"def tl_i64(n):
    return ((n + 9223372036854775808) % 18446744073709551616) - 9223372036854775808
"#;

/// Python floors, and `int(a / b)` would truncate through a float and lose bits on the way. The
/// remainder takes its sign from the dividend, so `a == tl_div(a, b) * b + tl_rem(a, b)` holds.
const ARITH_HELPER: &str = r#"def tl_div(a, b):
    if b == 0:
        tl_fail("divided by zero")
    q = abs(a) // abs(b)
    return tl_i32(-q if (a < 0) != (b < 0) else q)


def tl_rem(a, b):
    if b == 0:
        tl_fail("divided by zero")
    r = abs(a) % abs(b)
    return -r if a < 0 else r
"#;

/// `tl_div`/`tl_rem` at 64 bits. Only the quotient can leave the range (`MIN / -1`), so the
/// remainder needs no wrap: its magnitude is strictly below the divisor's.
const ARITH64_HELPER: &str = r#"def tl_div64(a, b):
    if b == 0:
        tl_fail("divided by zero")
    q = abs(a) // abs(b)
    return tl_i64(-q if (a < 0) != (b < 0) else q)


def tl_rem64(a, b):
    if b == 0:
        tl_fail("divided by zero")
    r = abs(a) % abs(b)
    return -r if a < 0 else r
"#;

const FIELD_HELPER: &str = r#"def tl_field(v, k, depth):
    if depth == 0:
        return v[k]
    return [tl_field(e, k, depth - 1) for e in v]
"#;

/// An Opt is its enum's own runtime shape (ADR 0009): `{"some": v}` present, `"none"` absent.
/// Tagged, so two levels of absence stay two values; only the printer flattens to null.
const AT_HELPER: &str = r#"def tl_at(v, i, depth):
    if depth > 0:
        return [tl_at(e, i, depth - 1) for e in v]
    n = len(v)
    if i < 0:
        i = n + i
    if i < 0 or i >= n:
        return "none"
    return {"some": v[i]}
"#;

const UNWRAP_HELPER: &str = r#"def tl_unwrap(v, depth):
    if depth > 0:
        return [tl_unwrap(e, depth - 1) for e in v]
    if v == "none":
        tl_fail("unwrapped a value that is not there")
    return v["some"]
"#;

const TAIL_HELPER: &str = r#"def tl_tail(v):
    if len(v) == 0:
        return "none"
    return {"some": v[1:]}
"#;

const FLATTEN_HELPER: &str = r#"def tl_flatten(vv):
    return [e for sub in vv for e in sub]
"#;

const COLLECT_HELPER: &str = r#"def tl_collect_lines():
    out = []
    for line in sys.stdin:
        out.append(line[:-1] if line.endswith("\n") else line)
    return out
"#;

const RANGE_HELPER: &str = r#"def tl_range(n):
    return list(range(max(0, n)))
"#;

/// Python 3 strings are already sequences of Unicode scalar values, so iterating one already
/// decodes by codepoint; `ord` is the cast down to the number every other backend represents a
/// `Char` as.
const CHARS_HELPER: &str = r#"def tl_chars(s):
    return [ord(c) for c in s]
"#;

const JOIN_HELPER: &str = r#"def tl_join(v, f):
    return "[" + ",".join(f(e) for e in v) + "]"
"#;

const JSONLINES_HELPER: &str = r#"def tl_jsonlines(v, f):
    return "\n".join(f(e) for e in v)
"#;

/// Iterating characters rather than bytes, which agrees with the C runtime's byte loop because
/// the two differ only above U+007F, where both pass the value through unchanged.
const QUOTE_HELPER: &str = r#"def tl_quote(s):
    out = ['"']
    for c in s:
        if c == '"':
            out.append('\\"')
        elif c == "\\":
            out.append("\\\\")
        elif c == "\n":
            out.append("\\n")
        elif c == "\r":
            out.append("\\r")
        elif c == "\t":
            out.append("\\t")
        elif ord(c) < 0x20:
            out.append("\\u%04x" % ord(c))
        else:
            out.append(c)
    out.append('"')
    return "".join(out)
"#;

pub fn emit(program: &Program) -> String {
    let enums = &program.enums;
    // Before the program's own functions, so a printer is defined by the time anything calls
    // one, and so the helper scan below sees what a printer body uses.
    let mut decls = printers(program);

    // Module-level functions are looked up when called, not when defined, so the forward
    // reference the checker accepts costs nothing here. Lua needed declarations, JavaScript
    // relied on hoisting, and jq could not express it at all.
    for f in &program.funcs {
        decls.push_str(&format!(
            "def {}({}):\n    return {}\n\n\n",
            user(&f.name),
            f.param.as_deref().map_or_else(String::new, user),
            expr(enums, &f.body)
        ));
    }

    if let Some(fusion) = tir::fusion(program) {
        decls.push_str(&fused_main(program, &fusion));
    } else {
        if program.input.is_some() {
            decls.push_str(&format!(
                "{INPUT} = json.loads(sys.stdin.buffer.read().decode(\"utf-8\"))\n"
            ));
        }
        if program.inputs.is_some() {
            decls.push_str(&format!(
                "{INPUTS} = [json.loads(_l) for _l in sys.stdin]\n"
            ));
        }

        let body = expr(enums, &program.body);
        // A top-level Str prints raw, the way jq's -r does; anything else prints as JSON.
        let printed = if program.body.ty == Type::Str {
            body
        } else {
            show(enums, &program.body.ty, &body, 0)
        };
        // Bytes rather than `print`, so output does not depend on the locale the interpreter was
        // started under. The native backend writes bytes for the same reason.
        decls.push_str(&format!(
            "sys.stdout.buffer.write(({printed} + \"\\n\").encode(\"utf-8\"))\n"
        ));
    }

    // Which helpers to include is read off the emitted text. Python objects to neither an unused
    // function nor an unused import, so nothing here can break the way it would in Go; the
    // helper-to-helper edges are still stated rather than read, since inclusion is what pulls
    // them in.
    let uses = |name: &str| decls.contains(name);
    let unwrap = uses("tl_unwrap(");
    let arith = uses("tl_div(") || uses("tl_rem(");
    let arith64 = uses("tl_div64(") || uses("tl_rem64(");

    let mut helpers = false;
    let mut out = String::from("import sys\n");
    if program.input.is_some() || program.inputs.is_some() {
        out.push_str("import json\n");
    }
    out.push('\n');
    for (on, text) in [
        (unwrap || arith || arith64, FAIL_HELPER),
        (arith || uses("tl_i32("), I32_HELPER),
        (arith64 || uses("tl_i64("), I64_HELPER),
        (arith, ARITH_HELPER),
        (arith64, ARITH64_HELPER),
        (uses("tl_field("), FIELD_HELPER),
        (uses("tl_at("), AT_HELPER),
        (uses("tl_tail("), TAIL_HELPER),
        (uses("tl_flatten("), FLATTEN_HELPER),
        (unwrap, UNWRAP_HELPER),
        (uses("tl_range("), RANGE_HELPER),
        (uses("tl_chars("), CHARS_HELPER),
        (uses("tl_collect_lines("), COLLECT_HELPER),
        (uses("tl_join("), JOIN_HELPER),
        (uses("tl_jsonlines("), JSONLINES_HELPER),
        (uses("tl_quote("), QUOTE_HELPER),
    ] {
        if on {
            out.push_str("\n\n");
            out.push_str(text);
            helpers = true;
        }
    }
    if helpers {
        out.push_str("\n\n");
    }
    out.push_str(&decls);
    out
}

/// A stream-typed `jsonlines` program, compiled as a loop reading one entry at a time from
/// `sys.stdin` (which iterates lazily) rather than the eager path's read-everything-first.
/// `sys.stdout.buffer` is block-buffered whenever it is not a terminal, the same as the eager
/// path already writes bytes rather than using `print` for, so the explicit `.flush()` after
/// each line is what makes a record appear before the next one arrives rather than after the
/// whole run ends.
fn fused_main(program: &Program, fusion: &tir::Fusion) -> String {
    let enums = &program.enums;
    let mut out = String::new();
    out.push_str("for _line in sys.stdin:\n");
    out.push_str("    _line = _line[:-1] if _line.endswith(\"\\n\") else _line\n");
    let (mut current, mut current_ty) = match fusion.source {
        tir::Source::Inputs => {
            out.push_str("    if _line.strip() == \"\":\n        continue\n");
            out.push_str("    t_line = json.loads(_line)\n");
            let elem = program
                .inputs
                .as_ref()
                .expect("an inputs source recorded its element");
            ("t_line".to_string(), elem.clone())
        }
        // A raw line is already the element, blank ones included: `lines` keeps them.
        tir::Source::Lines => ("_line".to_string(), Type::Str),
    };
    for stage in &fusion.stages {
        match stage {
            tir::Stage::Map { param, body } => {
                out.push_str(&format!("    {} = {}\n", local(*param), current));
                current = expr(enums, body);
                current_ty = body.ty.clone();
            }
            tir::Stage::Select { param, pred } => {
                out.push_str(&format!("    {} = {}\n", local(*param), current));
                out.push_str(&format!(
                    "    if not ({}):\n        continue\n",
                    expr(enums, pred)
                ));
                current = local(*param);
            }
        }
    }
    let printed = show(enums, &current_ty, &current, 0);
    out.push_str(&format!(
        "    sys.stdout.buffer.write(({printed} + \"\\n\").encode(\"utf-8\"))\n"
    ));
    out.push_str("    sys.stdout.buffer.flush()\n");
    out
}

/// The printer is built from the type rather than by inspecting the value, as on every backend.
/// Here the type is doing one thing it does nowhere else: `str` on a Bool gives `True`, so the
/// two JSON words have to be written out.
fn show(enums: &Enums, ty: &Type, value: &str, depth: usize) -> String {
    match ty {
        Type::Param(_) => unreachable!("params are substituted before emit"),
        // The checker refuses a program whose result contains a stream, since there is nothing to
        // print: a stream has no value, only a promise that collect can redeem.
        Type::Stream(_) => unreachable!("a stream cannot reach the printer"),
        Type::Char => unreachable!("Char cannot reach the printer, refused by the checker"),
        Type::Str => format!("tl_quote({value})"),
        Type::Int | Type::Int64 => format!("str({value})"),
        Type::Bool => format!("(\"true\" if {value} else \"false\")"),
        Type::Vec(elem) => {
            let e = format!("e{depth}");
            format!(
                "tl_join({value}, lambda {e}: {})",
                show(enums, elem, &e, depth + 1)
            )
        }
        Type::Enum { .. } if ty.as_opt().is_some() => {
            let inner = ty.as_opt().expect("guarded");
            let v = format!("o{depth}");
            format!(
                "(lambda {v}: \"null\" if {v} == \"none\" else {})({value})",
                show(enums, inner, &format!("{v}[\"some\"]"), depth + 1)
            )
        }
        // A recursive enum prints through a function of its own (`printers`), because expanding
        // one here has no bottom: its payload leads back to the same type.
        Type::Enum { .. } if ty::is_recursive(enums, ty) => format!("{}({value})", ty.show_fn()),
        Type::Enum { .. } => show_enum(enums, ty, value, depth),
        Type::Record(fields) => {
            if fields.is_empty() {
                return "\"{}\"".to_string();
            }
            // The type's field list is declaration order, so this prints as declared. Field
            // names are identifiers, so the JSON key needs no escaping and is one literal.
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, fty)| {
                    let read = format!("{value}[{}]", py_string(name));
                    let key = py_string(&format!("\"{name}\":"));
                    format!("{key} + {}", show(enums, fty, &read, depth + 1))
                })
                .collect();
            format!("(\"{{\" + {} + \"}}\")", parts.join(" + \",\" + "))
        }
    }
}

fn user(name: &str) -> String {
    format!("v_{name}")
}

fn local(id: LocalId) -> String {
    format!("t_{id}")
}

fn expr(enums: &Enums, t: &Tir) -> String {
    match &t.kind {
        Kind::Str(s) => py_string(s),
        Kind::Int(n) => n.to_string(),
        Kind::Var(name) => user(name),
        Kind::Local(id) => local(*id),
        Kind::Input => INPUT.to_string(),
        Kind::Inputs => INPUTS.to_string(),
        // The stream, materialized eagerly: whatever consumes it -- `collect`, a mapper --
        // works on the Vec of its entries. Fusion is what will remove this materialization.
        Kind::Lines => "tl_collect_lines()".to_string(),
        Kind::RecordLit { fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| format!("{}: {}", py_string(name), expr(enums, value)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }

        // The value is its JSON shape: a unit variant is the variant-name string, a payload
        // variant the single-key dict a record already is.
        Kind::EnumLit { variant, payload } => match payload {
            None => py_string(variant),
            Some(p) => format!("{{{}: {}}}", py_string(variant), expr(enums, p)),
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
        // Python's own `+` already concatenates two lists the way it concatenates two strings,
        // so a Vec needs no different spelling here than Str does.
        Kind::Concat(l, r) => format!("({} + {})", expr(enums, l), expr(enums, r)),
        Kind::Arith { op, lhs, rhs } => arith(&t.ty, *op, expr(enums, lhs), expr(enums, rhs)),
        // The one construct this target spells exactly as toylang does, because toylang took the
        // spelling from here.
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => {
            format!(
                "({} if {} else {})",
                expr(enums, then),
                expr(enums, cond),
                expr(enums, otherwise)
            )
        }
        Kind::Builtin { which, arg } => match which {
            Builtin::IntToStr => format!("str({})", expr(enums, arg)),
            // Python's integers are one type at every width, so the bridge has nothing to do.
            Builtin::IntToI64 => expr(enums, arg),
            Builtin::Range => format!("tl_range({})", expr(enums, arg)),
            Builtin::Chars => format!("tl_chars({})", expr(enums, arg)),
            Builtin::JsonLines => {
                let elem = tir::runtime_elem(&arg.ty).expect("checked to be a Vec or a stream");
                let e = "e0".to_string();
                format!(
                    "tl_jsonlines({}, lambda {e}: {})",
                    expr(enums, arg),
                    show(enums, elem, &e, 1)
                )
            }
            // The source already materialized, so the exit has nothing left to do.
            Builtin::Collect => expr(enums, arg),
            Builtin::Length => format!("len({})", expr(enums, arg)),
            Builtin::Tail => format!("tl_tail({})", expr(enums, arg)),
            Builtin::Flatten => format!("tl_flatten({})", expr(enums, arg)),
            // Python compares both numbers and strings (by codepoint) with `<` natively, so
            // `sorted` needs no key or comparator.
            Builtin::Sort => format!("sorted({})", expr(enums, arg)),
            Builtin::Reverse => format!("({})[::-1]", expr(enums, arg)),
            // The names come from the checked type, not the dict value, so `arg` runs as the
            // lambda's ignored argument -- the same shape `Bind` uses -- purely for whatever
            // else it does.
            Builtin::Fields => {
                let Type::Record(fields) = &arg.ty else {
                    unreachable!("checked to be a record")
                };
                let names: Vec<String> = fields.iter().map(|(n, _)| py_string(n)).collect();
                format!("(lambda _: [{}])({})", names.join(", "), expr(enums, arg))
            }
        },
        Kind::Compare { op, lhs, rhs } => {
            format!("({} {} {})", expr(enums, lhs), py_op(*op), expr(enums, rhs))
        }
        // Python spells and short-circuits both of these the way toylang does.
        Kind::Logic { op, lhs, rhs } => format!("({} {op} {})", expr(enums, lhs), expr(enums, rhs)),
        Kind::Not(base) => format!("(not {})", expr(enums, base)),
        Kind::Bind {
            local: id,
            value,
            body,
        } => {
            format!(
                "(lambda {}: {})({})",
                local(*id),
                expr(enums, body),
                expr(enums, value)
            )
        }
        // A comprehension rather than `map`, which returns an iterator here and would need a
        // `list` around it anyway.
        Kind::Map {
            source,
            param,
            body,
        } => {
            format!(
                "[{} for {} in {}]",
                expr(enums, body),
                local(*param),
                expr(enums, source)
            )
        }
        Kind::Select {
            source,
            param,
            pred,
        } => {
            let p = local(*param);
            format!(
                "[{p} for {p} in {} if {}]",
                expr(enums, source),
                expr(enums, pred)
            )
        }
        Kind::Unwrap { base } => {
            format!(
                "tl_unwrap({}, {})",
                expr(enums, base),
                tir::vec_depth(&base.ty)
            )
        }
        // Opt's reorder pass (kantord/toylang#66): the same `== "none"`/`["some"]` shape the
        // printer and Match already read, generalised to rebuild the dict instead.
        Kind::OptMap {
            source,
            param,
            body,
        } => {
            format!(
                "(lambda __opt: \"none\" if __opt == \"none\" else {{\"some\": (lambda {}: {})(__opt[\"some\"])}})({})",
                local(*param),
                expr(enums, body),
                expr(enums, source)
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
        Kind::Field { base, name } => {
            let depth = tir::vec_depth(&base.ty);
            if depth == 0 {
                format!("{}[{}]", expr(enums, base), py_string(name))
            } else {
                format!(
                    "tl_field({}, {}, {})",
                    expr(enums, base),
                    py_string(name),
                    depth
                )
            }
        }
        // A right-fold of conditional expressions over the subject, tests first-match-wins; a
        // guard arm's test is the guard itself. A total chain's last arm carries no test, the
        // checker having proved nothing else can reach it; a partial chain tests every arm and
        // bottoms out at `None`, the absent Opt. The payload key test needs the isinstance
        // guard: `"a" in v` on a unit variant's *string* would be a substring test, and
        // `"a" in "ab"` answers yes.
        Kind::Match {
            subject,
            arms,
            partial,
        } => {
            let subj = expr(enums, subject);
            let mut out = String::new();
            let mut closing = 0;
            for (i, arm) in arms.iter().enumerate() {
                let run = match arm.payload {
                    Some(pid) => {
                        let variant = arm
                            .variant
                            .as_ref()
                            .expect("only a variant arm has a payload");
                        format!(
                            "(lambda {}: {})({subj}[{}])",
                            local(pid),
                            expr(enums, &arm.body),
                            py_string(variant)
                        )
                    }
                    None => expr(enums, &arm.body),
                };
                let test = match (&arm.variant, &arm.guard) {
                    (Some(v), _) if arm.payload.is_some() => Some(format!(
                        "(isinstance({subj}, dict) and {} in {subj})",
                        py_string(v)
                    )),
                    (Some(v), _) => Some(format!("{subj} == {}", py_string(v))),
                    (None, Some(g)) => Some(expr(enums, g)),
                    (None, None) => None,
                };
                // A partial chain's yield is an Opt, so a present arm is tagged.
                let run = if *partial {
                    format!("{{\"some\": {run}}}")
                } else {
                    run
                };
                match test {
                    Some(test) if *partial || i + 1 < arms.len() => {
                        out.push_str(&format!("({run} if {test} else "));
                        closing += 1;
                    }
                    _ => out.push_str(&run),
                }
            }
            if *partial {
                out.push_str("\"none\"");
            }
            out.push_str(&")".repeat(closing));
            format!("({out})")
        }
    }
}

/// One arithmetic expression at the width the node's type names (kantord/toylang#83): the
/// same emulation either way, through the 64-bit helpers when the type says so.
fn arith(ty: &Type, op: BinOp, l: String, r: String) -> String {
    if *ty == Type::Int64 {
        match op {
            BinOp::Div => format!("tl_div64({l}, {r})"),
            BinOp::Rem => format!("tl_rem64({l}, {r})"),
            BinOp::Add => format!("tl_i64({l} + {r})"),
            BinOp::Sub => format!("tl_i64({l} - {r})"),
            BinOp::Mul => format!("tl_i64({l} * {r})"),
            other => unreachable!("{other} is not arithmetic"),
        }
    } else {
        match op {
            BinOp::Div => format!("tl_div({l}, {r})"),
            BinOp::Rem => format!("tl_rem({l}, {r})"),
            BinOp::Add => format!("tl_i32({l} + {r})"),
            BinOp::Sub => format!("tl_i32({l} - {r})"),
            BinOp::Mul => format!("tl_i32({l} * {r})"),
            other => unreachable!("{other} is not arithmetic"),
        }
    }
}

fn py_op(op: BinOp) -> &'static str {
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

/// The printer for one enum, inline. One type, two runtime shapes (ADR 0009): a unit variant is
/// a bare string, a payload variant a single-key dict, so the shape is inspected before
/// rendering. Which payload follows which key is still the type's knowledge, as everywhere else.
fn show_enum(enums: &Enums, ty: &Type, value: &str, depth: usize) -> String {
    let variants = ty::variants(enums, ty);
    let n = format!("n{depth}");
    let payloads: Vec<&(String, Option<Type>)> =
        variants.iter().filter(|(_, p)| p.is_some()).collect();
    if payloads.is_empty() {
        return format!("tl_quote({value})");
    }
    let mut body = String::new();
    if payloads.len() < variants.len() {
        body.push_str(&format!("tl_quote({n}) if isinstance({n}, str) else "));
    }
    for (i, (vname, pty)) in payloads.iter().enumerate() {
        let pty = pty.as_ref().expect("filtered to payload variants");
        let read = format!("{n}[{}]", py_string(vname));
        let wrapped = format!(
            "({} + {} + \"}}\")",
            py_string(&format!("{{\"{vname}\":")),
            show(enums, pty, &read, depth + 1)
        );
        if i + 1 < payloads.len() {
            body.push_str(&format!("{wrapped} if {} in {n} else ", py_string(vname)));
        } else {
            // The last payload variant needs no test: the type says nothing else is left.
            body.push_str(&wrapped);
        }
    }
    format!("(lambda {n}: {body})({value})")
}

/// A named printer for every recursive enum the program prints. The call in `show` above is
/// what a nested occurrence renders as, so the recursion in the type becomes recursion in the
/// emitted function rather than in this compiler (kantord/toylang#94).
fn printers(program: &Program) -> String {
    let mut out = String::new();
    for ty in tir::printed_recursive_enums(program) {
        out.push_str(&format!(
            "def {}(v):\n    return {}\n\n\n",
            ty.show_fn(),
            show_enum(&program.enums, &ty, "v", 0)
        ));
    }
    out
}

fn py_string(s: &str) -> String {
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
