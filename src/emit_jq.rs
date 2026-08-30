//! The jq backend.
//!
//! The other three targets are imperative and this one is not, so it is the only backend where
//! the compiler has to say what it means in stream terms. That makes it a check on the
//! relationship to jq rather than another way to run a program: everything the design diverges
//! on has to be bridged here explicitly.
//!
//! Two mappings are worth naming. A dimension spec becomes `.[]` plus a reification, since
//! keeping a dimension in a stream language means iterating and collecting. And `Opt` carries
//! its enum's own runtime shape even here -- `{"some": v}` present, `"none"` absent -- tagged
//! in memory like every backend now, with `null` appearing only when the printer flattens the
//! tags away. One consequence is load-bearing: no in-memory value is ever JSON null, so jq's
//! own null (an out-of-range `.[i]`) still unambiguously means "was not there".

use crate::ast::BinOp;
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::Type;

/// The binding stdin is read into. jq puts the input in `.`, which this backend needs for the
/// subject, so it is bound away before the program starts.
const INPUT: &str = "$t_input";
const INPUTS: &str = "$t_inputs";

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

/// Int64 arithmetic on doubles, exact only within +/-2^53 (kantord/toylang#83). That boundary
/// is documented rather than papered over: 32-bit wrapping could be emulated exactly through
/// 16-bit halves because every partial product fit a double, and no such split reassembles a
/// 64-bit product or a mod-2^64 wrap from pieces a double can hold. So `+`, `-` and `*` are
/// jq's own, `%` casts through C's 64-bit integers inside jq itself, and a program whose
/// Int64 values stay within 2^53 -- every corpus case does -- agrees with the other six
/// backends; one that leaves it diverges here, and docs/reference/types/int64.md says so.
const ARITH64_HELPER: &str = r#"def tl_div64($a; $b):
  if $b == 0 then error("toylang: divided by zero") else ($a / $b | trunc) end;
def tl_rem64($a; $b):
  if $b == 0 then error("toylang: divided by zero") else ($a % $b) end;
"#;

pub fn emit(program: &Program) -> Result<String, String> {
    let mut out = String::new();
    let (arith, arith64) = uses_arith(program);
    if arith {
        out.push_str(ARITH_HELPER);
    }
    if arith64 {
        out.push_str(ARITH64_HELPER);
    }

    // jq resolves a `def` only against what is already defined, so definitions have to come out
    // callee-first. The checker collects every signature before checking any body and therefore
    // accepts a call to a function defined further down, which is a rule this target does not
    // share. Lua needed forward declarations for the same reason; jq has no way to write one.
    for f in ordered(program)? {
        // A unary function's argument arrives as `.` and is bound before the body runs; a
        // nullary one ignores `.` entirely, since it has nothing to bind.
        out.push_str(&match &f.param {
            Some(param) => format!(
                "def {}: . as ${} | {};\n",
                user(&f.name),
                user(param),
                expr(&f.body)
            ),
            None => format!("def {}: {};\n", user(&f.name), expr(&f.body)),
        });
    }

    if let Some(fusion) = tir::fusion(program) {
        // jq's own `inputs` is already lazy; the eager path below only becomes eager by wrapping
        // it in `[...]`. Skipping that wrapper and running the whole `map`/`select` chain as one
        // filter over the `inputs` generator is what makes jq print each record as it arrives
        // rather than after the last one, the same as running the equivalent `jq` program by
        // hand would.
        out.push_str("inputs");
        for stage in &fusion.stages {
            match stage {
                tir::Stage::Map { param, body } => {
                    out.push_str(&format!(" | . as {} | {}", local(*param), expr(body)));
                }
                tir::Stage::Select { param, pred } => {
                    out.push_str(&format!(
                        " | . as {} | select({})",
                        local(*param),
                        expr(pred)
                    ));
                }
            }
        }
        let Kind::Builtin { arg, .. } = &program.body.kind else {
            unreachable!("fusion only matches a jsonlines body")
        };
        let elem = tir::runtime_elem(&arg.ty).expect("jsonlines's argument has an element");
        out.push_str(&format!(" | ({} | tojson)\n", canonical(elem, ".")));
        return Ok(out);
    }

    if program.input.is_some() {
        out.push_str(&format!(". as {INPUT} | "));
    }
    // `-n` (added whenever the invocation has no value already in `.`, run_jq's job) is what
    // makes `inputs` mean "every value on stdin" rather than "every value after the first."
    if program.inputs.is_some() {
        out.push_str(&format!("[inputs] as {INPUTS} | "));
    }
    // Records are rebuilt in the type's field order, because jq preserves insertion order and
    // an object read from input carries whatever order the input had.
    out.push_str(&canonical(&program.body.ty, &expr(&program.body)));
    out.push('\n');
    Ok(out)
}

/// Reconstruct a value with keys in the type's order, so the printed form matches the other
/// backends rather than the input's key order.
fn canonical(ty: &Type, value: &str) -> String {
    match ty {
        Type::Param(_) => unreachable!("params are substituted before emit"),
        // The checker refuses a program whose result contains a stream, since there is nothing to
        // print: a stream has no value, only a promise that collect can redeem.
        Type::Stream(_) => unreachable!("a stream cannot reach the printer"),
        Type::Char => unreachable!("Char cannot reach the printer, refused by the checker"),
        Type::Str | Type::Int | Type::Int64 | Type::Bool => value.to_string(),
        Type::Vec(elem) => format!("[ {value}[] | {} ]", canonical(elem, ".")),
        Type::Enum { .. } if ty.as_opt().is_some() => {
            let inner = ty.as_opt().expect("guarded");
            format!(
                "({value} | if . == \"none\" then null else (.some | {}) end)",
                canonical(inner, ".")
            )
        }
        // One type, two runtime shapes (ADR 0009). A unit variant is already canonical, being a
        // bare string; a payload wrapper is rebuilt so the payload's own keys come out in the
        // type's order, the same reason records are rebuilt.
        Type::Enum { variants, .. } => {
            let payloads: Vec<(&String, &Type)> = variants
                .iter()
                .filter_map(|(n, p)| p.as_ref().map(|p| (n, p)))
                .collect();
            if payloads.is_empty() {
                return value.to_string();
            }
            let wrap = |vname: &String, pty: &Type| {
                format!(
                    "{{{}: {}}}",
                    jq_string(vname),
                    canonical(pty, &field_of(".", vname))
                )
            };
            // The conditions cover everything but the last shape, which needs no test: the type
            // says nothing else is left. A unit variant, if any exists, is the string shape.
            let mut tests: Vec<String> = Vec::new();
            if payloads.len() < variants.len() {
                tests.push("if type == \"string\" then .".to_string());
            }
            for (vname, pty) in &payloads[..payloads.len() - 1] {
                let word = if tests.is_empty() { "if" } else { "elif" };
                tests.push(format!(
                    "{word} has({}) then {}",
                    jq_string(vname),
                    wrap(vname, pty)
                ));
            }
            let (last_name, last_ty) = payloads[payloads.len() - 1];
            let last = wrap(last_name, last_ty);
            if tests.is_empty() {
                return format!("({value} | {last})");
            }
            format!("({value} | {} else {last} end)", tests.join(" "))
        }
        Type::Record(fields) => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, fty)| {
                    format!(
                        "{}: {}",
                        jq_string(name),
                        canonical(fty, &field_of(".", name))
                    )
                })
                .collect();
            format!("({value} | {{{}}})", parts.join(", "))
        }
    }
}

/// Every function name a `Kind::Call` inside `t` names, in the order encountered, duplicates and
/// all -- `ordered` only ever asks whether a name is present, and a cycle's error message reads
/// better walking a real call rather than a deduplicated set.
fn callees(t: &Tir, out: &mut Vec<String>) {
    match &t.kind {
        Kind::Str(_)
        | Kind::Int(_)
        | Kind::Var(_)
        | Kind::Local(_)
        | Kind::Input
        | Kind::Inputs
        | Kind::Lines => {}
        Kind::VecLit(items) => items.iter().for_each(|i| callees(i, out)),
        Kind::RecordLit { fields } => {
            fields.iter().for_each(|(_, v)| callees(v, out));
        }
        Kind::EnumLit { payload, .. } => {
            if let Some(p) = payload {
                callees(p, out);
            }
        }
        Kind::Call { func, arg } => {
            out.push(func.clone());
            if let Some(a) = arg {
                callees(a, out);
            }
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
        Kind::Map { source, body, .. } | Kind::OptMap { source, body, .. } => {
            callees(source, out);
            callees(body, out);
        }
        Kind::Builtin { arg, .. } => callees(arg, out),
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => {
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
        Kind::Match { subject, arms, .. } => {
            callees(subject, out);
            for a in arms {
                if let Some(g) = &a.guard {
                    callees(g, out);
                }
                callees(&a.body, out);
            }
        }
    }
}

/// Definitions in callee-before-caller order, or the cycle blocking one: jq's `def` sees only
/// itself and whatever is already defined above it, with no forward declaration to bridge a
/// real cycle between two or more named functions (kantord/toylang#79). Self-recursion never
/// gets stuck here -- a function calling only itself is always immediately ready -- so reaching
/// the stuck state below means the remaining functions have a genuine cycle among them.
fn ordered(program: &Program) -> Result<Vec<&tir::Func>, String> {
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
                calls.iter().all(|c| c == &f.name || placed.contains(c))
            })
            .map(|(i, _)| i)
            .collect();
        if ready.is_empty() {
            return Err(cycle_message(&remaining));
        }
        for i in ready.into_iter().rev() {
            let f = remaining.remove(i);
            placed.push(f.name.clone());
            done.push(f);
        }
    }
    Ok(done)
}

/// A concrete cycle among `remaining`'s functions, walked by following one unresolved call from
/// each function to the next: every function still in `remaining` has at least one such call
/// (otherwise it would have been ready), so the walk is finite and must revisit a name.
fn cycle_message(remaining: &[&tir::Func]) -> String {
    let names: Vec<&str> = remaining.iter().map(|f| f.name.as_str()).collect();
    let mut path: Vec<String> = Vec::new();
    let mut current = remaining[0].name.clone();
    loop {
        if let Some(at) = path.iter().position(|n| *n == current) {
            let cycle = &path[at..];
            let chain: Vec<String> = cycle.iter().map(|n| format!("`{n}`")).collect();
            return format!(
                "jq cannot compile this: {} -> `{}` is a cycle between named functions, and \
                 jq's `def` has no forward declaration -- only self-recursion and a call to \
                 something already defined above it are representable here",
                chain.join(" -> "),
                cycle[0],
            );
        }
        path.push(current.clone());
        let f = remaining
            .iter()
            .find(|f| f.name == current)
            .expect("current is a name drawn from remaining");
        let mut calls = Vec::new();
        callees(&f.body, &mut calls);
        current = calls
            .into_iter()
            .find(|c| c != &f.name && names.contains(&c.as_str()))
            .expect("a stuck function has at least one unresolved call within remaining");
    }
}

/// Whether the program does 32-bit arithmetic, 64-bit arithmetic, or both: the node's type
/// says which width each `Arith` is, so the two helper blocks are included independently.
fn uses_arith(program: &Program) -> (bool, bool) {
    fn walk(t: &Tir, found: &mut (bool, bool)) {
        match &t.kind {
            Kind::Arith { lhs, rhs, .. } => {
                if t.ty == Type::Int64 {
                    found.1 = true;
                } else {
                    found.0 = true;
                }
                walk(lhs, found);
                walk(rhs, found);
            }
            Kind::Cond {
                cond,
                then,
                otherwise,
            } => {
                walk(cond, found);
                walk(then, found);
                walk(otherwise, found);
            }
            Kind::Str(_)
            | Kind::Int(_)
            | Kind::Var(_)
            | Kind::Local(_)
            | Kind::Input
            | Kind::Inputs
            | Kind::Lines => {}
            Kind::VecLit(items) => items.iter().for_each(|i| walk(i, found)),
            Kind::RecordLit { fields } => fields.iter().for_each(|(_, v)| walk(v, found)),
            Kind::EnumLit { payload, .. } => {
                if let Some(p) = payload {
                    walk(p, found);
                }
            }
            Kind::Call { arg, .. } => {
                if let Some(a) = arg {
                    walk(a, found);
                }
            }
            Kind::Builtin { arg, .. } => walk(arg, found),
            Kind::Concat(l, r) | Kind::Compare { lhs: l, rhs: r, .. } => {
                walk(l, found);
                walk(r, found);
            }
            Kind::Bind { value, body, .. }
            | Kind::Map {
                source: value,
                body,
                ..
            }
            | Kind::OptMap {
                source: value,
                body,
                ..
            } => {
                walk(value, found);
                walk(body, found);
            }
            Kind::Select { source, pred, .. } => {
                walk(source, found);
                walk(pred, found);
            }
            Kind::Field { base, .. } | Kind::Unwrap { base } => walk(base, found),
            Kind::Index { base, index, .. } => {
                walk(base, found);
                walk(index, found);
            }
            Kind::Match { subject, arms, .. } => {
                walk(subject, found);
                for a in arms {
                    if let Some(g) = &a.guard {
                        walk(g, found);
                    }
                    walk(&a.body, found);
                }
            }
        }
    }
    let mut found = (false, false);
    program.funcs.iter().for_each(|f| walk(&f.body, &mut found));
    walk(&program.body, &mut found);
    found
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
        Kind::Inputs => INPUTS.to_string(),
        // The stream, materialized eagerly: `[ inputs ]` in raw-input mode is every line of
        // stdin as an array of strings. `-n -R` on the invocation is what makes this mode
        // available; see the checker rule against mixing `input` and `lines` in one program.
        Kind::Lines => "[ inputs ]".to_string(),
        // Each value is parenthesised: everything in jq is a filter, so an unbracketed `|`
        // or `,` inside one would be read as part of the object rather than as its value.
        Kind::RecordLit { fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| format!("{}: ({})", jq_string(name), expr(value)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }

        // The value is its JSON shape, which jq spells directly.
        Kind::EnumLit { variant, payload } => match payload {
            None => jq_string(variant),
            Some(p) => format!("{{{}: ({})}}", jq_string(variant), expr(p)),
        },

        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(expr).collect();
            format!("[{}]", parts.join(", "))
        }
        // A nullary function ignores `.`, so its call is just its name, run against whatever
        // is already flowing through the surrounding pipeline.
        Kind::Call { func, arg } => match arg {
            Some(arg) => format!("({} | {})", expr(arg), user(func)),
            None => format!("({})", user(func)),
        },
        Kind::Concat(l, r) => format!("({} + {})", expr(l), expr(r)),
        Kind::Arith { op, lhs, rhs } => arith(&t.ty, *op, expr(lhs), expr(rhs)),
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => format!(
            "(if {} then {} else {} end)",
            expr(cond),
            expr(then),
            expr(otherwise)
        ),
        Kind::Builtin { which, arg } => match which {
            Builtin::IntToStr => format!("({} | tostring)", expr(arg)),
            // jq has one number type at every width, so the bridge has nothing to do.
            Builtin::IntToI64 => format!("({})", expr(arg)),
            Builtin::Range => format!("[ range(0; {}) ]", expr(arg)),
            // `explode` already decodes jq's UTF-8 string by codepoint, not by byte, so there
            // is no decoding to get right here.
            Builtin::Chars => format!("({} | explode)", expr(arg)),
            // `canonical` reorders a record's keys but leaves the *value*, not text; `tojson`
            // is what turns each element into the same compact JSON string `-c` would print for
            // it, matching every other backend's per-element encoding.
            Builtin::JsonLines => {
                let elem = tir::runtime_elem(&arg.ty).expect("checked to be a Vec or a stream");
                format!(
                    "({} | [.[] | ({} | tojson)] | join(\"\\n\"))",
                    expr(arg),
                    canonical(elem, ".")
                )
            }
            // The source already materialized, so the exit has nothing left to do.
            Builtin::Collect => expr(arg),
            Builtin::Extent => format!("({} | length)", expr(arg)),
            // jq's own `.[1:]` on an empty array is `[]`, not null; toylang's tail needs the
            // tagged Opt shape instead, so both cases are spelled out rather than borrowed.
            Builtin::Tail => {
                format!(
                    "({} | if length == 0 then \"none\" else {{some: .[1:]}} end)",
                    expr(arg)
                )
            }
            // Not jq's own `add`, which is `null` on an empty list rather than `[]` -- a reduce
            // starting from `[]` gives the right answer in both cases.
            Builtin::Concat => format!("({} | reduce .[] as $x ([]; . + $x))", expr(arg)),
            // jq's own `sort`/`reverse` already are this, restricted the same way the checker
            // restricts `sort` to Int, Int64, Str, and Char.
            Builtin::Sort => format!("({} | sort)", expr(arg)),
            Builtin::Reverse => format!("({} | reverse)", expr(arg)),
            // The names come from the checked type, not the object value, so `arg` runs only to
            // become the `.` a literal array then ignores -- the same discard the pipe already
            // gives every other builtin here.
            Builtin::Fields => {
                let Type::Record(fields) = &arg.ty else {
                    unreachable!("checked to be a record")
                };
                let names: Vec<String> = fields.iter().map(|(n, _)| jq_string(n)).collect();
                format!("({} | [{}])", expr(arg), names.join(", "))
            }
        },
        Kind::Compare { op, lhs, rhs } => {
            format!("({} {} {})", expr(lhs), jq_op(*op), expr(rhs))
        }
        Kind::Bind {
            local: id,
            value,
            body,
        } => {
            format!("({} as {} | {})", expr(value), local(*id), expr(body))
        }
        // The one operator that is derived in jq and primitive here: `map(f)` is `[ .[] | f ]`
        // there, and neither half of that exists in a language with no effect layer.
        Kind::Map {
            source,
            param,
            body,
        } => format!(
            "[ {}[] | . as {} | {} ]",
            expr(source),
            local(*param),
            expr(body)
        ),
        Kind::Select {
            source,
            param,
            pred,
        } => format!(
            "[ {}[] | . as {} | select({}) ]",
            expr(source),
            local(*param),
            expr(pred)
        ),
        Kind::Field { base, name } => {
            let depth = tir::vec_depth(&base.ty);
            format!(
                "({} | {})",
                expr(base),
                distribute(&field_of(".", name), depth)
            )
        }
        Kind::Unwrap { base } => {
            let check = format!(
                "if . == \"none\" then error({}) else .some end",
                jq_string("toylang: unwrapped a value that is not there")
            );
            format!(
                "({} | {})",
                expr(base),
                distribute(&check, tir::vec_depth(&base.ty))
            )
        }
        // Opt's reorder pass (kantord/toylang#66): the same `== "none"`/`.some` shape the
        // printer and Match already read, generalised to rebuild the object instead.
        Kind::OptMap {
            source,
            param,
            body,
        } => format!(
            "({} | if . == \"none\" then \"none\" else (.some as {} | {{some: {}}}) end)",
            expr(source),
            local(*param),
            expr(body)
        ),
        // Out of range is null in jq, and no in-memory toylang value is ever null (the
        // module comment above), so the null test is exactly the was-not-there test; what
        // comes out is the tagged Opt either way.
        Kind::Index {
            base, index, depth, ..
        } => {
            let at = format!(
                "(.[{}] as $e | if $e == null then \"none\" else {{some: $e}} end)",
                expr(index)
            );
            format!("({} | {})", expr(base), distribute(&at, *depth))
        }
        // Tests over the subject: equality for a unit variant, `type`-guarded `has` for a
        // payload one, since `has` on a string is an error rather than false, and the guard's
        // own Bool for a guard arm. A total chain's last arm is the `else`, the checker having
        // proved nothing else can reach it; a partial chain tags every present arm and its
        // `else` is the absent Opt.
        Kind::Match {
            subject,
            arms,
            partial,
        } => {
            let subj = expr(subject);
            let run = |arm: &tir::MatchArm| match arm.payload {
                Some(pid) => {
                    let variant = arm
                        .variant
                        .as_ref()
                        .expect("only a variant arm has a payload");
                    format!(
                        "({subj}[{}] as {} | {})",
                        jq_string(variant),
                        local(pid),
                        expr(&arm.body)
                    )
                }
                None => expr(&arm.body),
            };
            let run = |arm: &tir::MatchArm| {
                if *partial {
                    // A partial chain's yield is an Opt, so a present arm is tagged.
                    format!("{{some: {}}}", run(arm))
                } else {
                    run(arm)
                }
            };
            if !*partial && arms.len() == 1 {
                return format!("({})", run(&arms[0]));
            }
            let mut out = String::from("(");
            for (i, arm) in arms.iter().enumerate() {
                let test = match (&arm.variant, &arm.guard) {
                    (Some(v), _) if arm.payload.is_some() => Some(format!(
                        "({subj} | type == \"object\" and has({}))",
                        jq_string(v)
                    )),
                    (Some(v), _) => Some(format!("{subj} == {}", jq_string(v))),
                    (None, Some(g)) => Some(expr(g)),
                    (None, None) => None,
                };
                match test {
                    Some(test) if *partial || i + 1 < arms.len() => {
                        let word = if i == 0 { "if" } else { "elif" };
                        out.push_str(&format!("{word} {test} then {} ", run(arm)));
                    }
                    _ => out.push_str(&format!("else {} end", run(arm))),
                }
            }
            if *partial {
                out.push_str("else \"none\" end");
            }
            out.push(')');
            out
        }
    }
}

/// One arithmetic expression at the width the node's type names. The 64-bit side stays in
/// jq's doubles, exact within +/-2^53 and honestly not past it (the ARITH64_HELPER comment);
/// nothing wraps there, since no double could carry the wrapped value back.
fn arith(ty: &Type, op: BinOp, l: String, r: String) -> String {
    if *ty == Type::Int64 {
        match op {
            BinOp::Div => format!("tl_div64({l}; {r})"),
            BinOp::Rem => format!("tl_rem64({l}; {r})"),
            BinOp::Mul => format!("({l} * {r})"),
            BinOp::Add => format!("({l} + {r})"),
            BinOp::Sub => format!("({l} - {r})"),
            other => unreachable!("{other} is not arithmetic"),
        }
    } else {
        match op {
            BinOp::Div => format!("tl_div({l}; {r})"),
            BinOp::Rem => format!("tl_rem({l}; {r})"),
            BinOp::Mul => format!("tl_mul({l}; {r})"),
            BinOp::Add => format!("(({l} + {r}) | tl_i32)"),
            BinOp::Sub => format!("(({l} - {r}) | tl_i32)"),
            other => unreachable!("{other} is not arithmetic"),
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
