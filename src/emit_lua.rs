use crate::ast::BinOp;
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::Type;

/// The global the input value is bound to before the chunk runs. Unspellable in source, since
/// every source name is prefixed.
pub const INPUT: &str = "t_input";
/// The global every remaining stdin line, already parsed, is bound to before the chunk runs.
pub const INPUTS: &str = "t_inputs";
/// The function `run_lua` injects in place of `INPUTS` for a fused, live-streamable program:
/// called with no arguments, it returns the next `inputs` record already parsed and converted,
/// or `nil` at EOF. See `tir::recognize_fusion` and this file's `fused_main`.
pub const NEXT_INPUT: &str = "tl_next_input";

const SELECT_HELPER: &str = "\
local function tl_select(src, pred)
  local out = {}
  for i = 1, #src do
    if pred(src[i]) then out[#out + 1] = src[i] end
  end
  return out
end
";

const FIELD_HELPER: &str = "\
local function tl_field(v, k, depth)
  if depth == 0 then return v[k] end
  local out = {}
  for i = 1, #v do out[i] = tl_field(v[i], k, depth - 1) end
  return out
end
";

const RANGE_HELPER: &str = "\
local function tl_range(n)
  local out = {}
  for i = 1, n do out[i] = i - 1 end
  return out
end
";

const COLLECT_HELPER: &str = "\
local function tl_collect_lines()
  local out = {}
  for line in io.lines() do out[#out + 1] = line end
  return out
end
";

const MAP_HELPER: &str = "\
local function tl_map(src, f)
  local out = {}
  for i = 1, #src do out[i] = f(src[i]) end
  return out
end
";

const OPT_HELPER: &str = "\
-- Absence is a sentinel rather than nil, because nil inside a table breaks `#` and a Vec of Opt
-- has to keep its length.
local tl_none = {}
local function tl_at(v, i, depth)
  if depth > 0 then
    local out = {}
    for k = 1, #v do out[k] = tl_at(v[k], i, depth - 1) end
    return out
  end
  local n = #v
  if i < 0 then i = n + i end
  if i < 0 or i >= n then return tl_none end
  return v[i + 1]
end
";

const TAIL_HELPER: &str = "\
local function tl_tail(v)
  if #v == 0 then return tl_none end
  local out = {}
  for i = 2, #v do out[i - 1] = v[i] end
  return out
end
";

const VEC_CONCAT_HELPER: &str = "\
local function tl_vec_concat(vv)
  local out = {}
  for i = 1, #vv do
    local inner = vv[i]
    for j = 1, #inner do out[#out + 1] = inner[j] end
  end
  return out
end
";

const UNWRAP_HELPER: &str = r#"local function tl_unwrap(v, depth)
  if depth > 0 then
    local out = {}
    for k = 1, #v do out[k] = tl_unwrap(v[k], depth - 1) end
    return out
  end
  if v == tl_none then error("toylang: unwrapped a value that is not there", 0) end
  return v
end
"#;

const ARITH_HELPER: &str = r#"local TL_2_31 <const> = 2147483648
local TL_2_32 <const> = 4294967296
-- Lua integers are 64-bit, so an Int has to be brought back into 32 bits after every operation.
-- `%` is floored here and the modulus is positive, so this is exact for negatives too.
local function tl_i32(x)
  return (x + TL_2_31) % TL_2_32 - TL_2_31
end
-- Lua's `//` floors and `%` is floored; toylang truncates, which math.fmod already does.
local function tl_div(a, b)
  if b == 0 then error("toylang: divided by zero", 0) end
  return tl_i32((a - math.fmod(a, b)) // b)
end
local function tl_rem(a, b)
  if b == 0 then error("toylang: divided by zero", 0) end
  return tl_i32(math.fmod(a, b))
end
"#;

const QUOTE_HELPER: &str = r#"local function tl_quote(s)
  return '"' .. s:gsub('[%c"\\]', function(c)
    if c == '"' then return '\\"' end
    if c == '\\' then return '\\\\' end
    if c == '\n' then return '\\n' end
    if c == '\r' then return '\\r' end
    if c == '\t' then return '\\t' end
    return string.format('\\u%04x', c:byte())
  end) .. '"'
end
"#;

const JOIN_HELPER: &str = "\
local function tl_join(v, f)
  local parts = {}
  for i = 1, #v do parts[i] = f(v[i]) end
  return \"[\" .. table.concat(parts, \",\") .. \"]\"
end
";

const JSONLINES_HELPER: &str = "\
local function tl_jsonlines(v, f)
  local parts = {}
  for i = 1, #v do parts[i] = f(v[i]) end
  return table.concat(parts, \"\\n\")
end
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
    // A top-level Str prints raw, the way jq's -r does; anything else prints as JSON. So a
    // string inside a Vec is quoted while a bare string is not.
    let structured = program.body.ty != Type::Str;
    if (structured && needs_quote(&program.body.ty)) || used.jsonlines {
        out.push_str(QUOTE_HELPER);
    }
    if (structured && contains_vec(&program.body.ty)) || used.jsonlines {
        out.push_str(JOIN_HELPER);
    }
    if used.index || used.unwrap || used.tail || contains_opt(&program.body.ty) {
        out.push_str(OPT_HELPER);
    }
    if used.unwrap {
        out.push_str(UNWRAP_HELPER);
    }
    if used.tail {
        out.push_str(TAIL_HELPER);
    }
    if used.concat {
        out.push_str(VEC_CONCAT_HELPER);
    }
    if used.arith {
        out.push_str(ARITH_HELPER);
    }
    if used.map {
        out.push_str(MAP_HELPER);
    }
    if used.range {
        out.push_str(RANGE_HELPER);
    }
    if used.collect {
        out.push_str(COLLECT_HELPER);
    }
    if used.jsonlines {
        out.push_str(JSONLINES_HELPER);
    }

    // All names are declared before any body, because the checker collects signatures before
    // checking bodies and so accepts a call to a function defined further down. Emitting
    // `local function` in source order would leave that call resolving to a nil global.
    if !program.funcs.is_empty() {
        let names: Vec<String> = program.funcs.iter().map(|f| user(&f.name)).collect();
        out.push_str(&format!("local {}\n", names.join(", ")));
    }

    for f in &program.funcs {
        out.push_str(&format!(
            "function {}({})\n  return {}\nend\n",
            user(&f.name),
            user(&f.param),
            expr(&f.body)
        ));
    }

    if let Some(fusion) = tir::recognize_fusion(program) {
        out.push_str(&fused_main(program, &fusion));
    } else {
        let body = expr(&program.body);
        if structured {
            out.push_str(&format!("print({})\n", show(&program.body.ty, &body, 0)));
        } else {
            out.push_str(&format!("print({body})\n"));
        }
    }
    out
}

/// A `jsonlines(f(inputs))` program, compiled as a loop calling `NEXT_INPUT` for one already-
/// parsed record at a time rather than reading a pre-populated `INPUTS` global -- see that
/// constant's doc comment for why the parsing itself is not written here.
fn fused_main(program: &Program, fusion: &tir::Fusion) -> String {
    let elem_ty = program.inputs.as_ref().expect("fusion only matches an `inputs` source");
    let mut out = String::new();
    out.push_str("while true do\n");
    out.push_str(&format!("  local t_line = {NEXT_INPUT}()\n"));
    out.push_str("  if t_line == nil then break end\n");

    let mut current = "t_line".to_string();
    let mut current_ty = elem_ty.clone();
    for stage in &fusion.stages {
        match stage {
            tir::Stage::Map { param, body } => {
                out.push_str(&format!("  local {} = {}\n", local(*param), current));
                current = expr(body);
                current_ty = body.ty.clone();
            }
            tir::Stage::Select { param, pred } => {
                out.push_str(&format!("  local {} = {}\n", local(*param), current));
                out.push_str(&format!(
                    "  if not ({}) then goto tl_continue end\n",
                    expr(pred)
                ));
                current = local(*param);
            }
        }
    }
    let printed = if current_ty == Type::Str { current } else { show(&current_ty, &current, 0) };
    out.push_str(&format!("  print({printed})\n"));
    out.push_str("  ::tl_continue::\n");
    out.push_str("end\n");
    out
}

/// The printer is built from the type rather than by inspecting the value. A Lua table cannot
/// say whether it is an array or a record -- an empty one is indistinguishable either way -- so
/// asking it was always going to disagree with a backend that knows. See emit_js for the same
/// function, and step 4 onwards, where a native target has nothing to ask.
fn show(ty: &Type, value: &str, depth: usize) -> String {
    match ty {
        // The checker refuses a program whose result contains Lines, since there is nothing to
        // print: a stream has no value, only a promise that collect can redeem.
        Type::Lines => unreachable!("Lines cannot reach the printer"),
        Type::Str => format!("tl_quote({value})"),
        Type::Int | Type::Bool => format!("tostring({value})"),
        Type::Vec(elem) => {
            let e = format!("e{depth}");
            format!("tl_join({value}, function({e}) return {} end)", show(elem, &e, depth + 1))
        }
        Type::Opt(inner) => {
            let v = format!("o{depth}");
            format!(
                "(function({v}) if {v} == tl_none then return \"null\" else return {} end end)({value})",
                show(inner, &v, depth + 1)
            )
        }
        Type::Record(fields) => {
            // `..` needs an operand on each side, so a record with no fields cannot be
            // built by joining nothing. The other backends survive this by construction.
            if fields.is_empty() {
                return "\"{}\"".to_string();
            }
            // Type::record keeps fields sorted, so this order is the type's order. Field names
            // are identifiers, so the JSON key needs no escaping and is one literal.
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, fty)| {
                    let read = format!("{value}[{}]", lua_string(name));
                    let key = lua_string(&format!("\"{name}\":"));
                    format!("{key} .. {}", show(fty, &read, depth + 1))
                })
                .collect();
            format!("(\"{{\" .. {} .. \"}}\")", parts.join(" .. \",\" .. "))
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

fn needs_quote(ty: &Type) -> bool {
    match ty {
        Type::Str => true,
        Type::Vec(elem) | Type::Opt(elem) => needs_quote(elem),
        Type::Record(_) => true,
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

/// Which identifiers are reserved is the target's business, not toylang's. A program with a
/// function called `print` or `end` would otherwise emit Lua that shadows the output function or
/// does not parse.
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
    map: bool,
    range: bool,
    collect: bool,
    jsonlines: bool,
    tail: bool,
    concat: bool,
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
                used.map = true;
                walk(source, used);
                walk(body, used);
            }
            Kind::Select { source, pred, .. } => {
                used.select = true;
                walk(source, used);
                walk(pred, used);
            }
            Kind::Field { base, .. } => {
                // Depth zero is a plain index and needs no helper.
                used.field |= tir::vec_depth(&base.ty) > 0;
                walk(base, used);
            }
            Kind::Builtin { which, arg } => {
                used.range |= *which == Builtin::Range;
                used.collect |= *which == Builtin::Collect;
                used.jsonlines |= *which == Builtin::JsonLines;
                used.tail |= *which == Builtin::Tail;
                used.concat |= *which == Builtin::Concat;
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
        Kind::Str(s) => lua_string(s),
        Kind::Int(n) => n.to_string(),
        Kind::Var(name) => user(name),
        Kind::Local(id) => local(*id),
        Kind::Input => INPUT.to_string(),
        Kind::Inputs => INPUTS.to_string(),
        // `lines` has no value of its own -- it is a promise that the real stdin has not been
        // read yet, made good only by `collect`. `nil` is never actually inspected.
        Kind::Lines => "nil".to_string(),
        // A record is a table keyed by field name, which is what field access reads.
        Kind::RecordLit { fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| format!("[{}] = {}", lua_string(name), expr(value)))
                .collect();
            // Parenthesised because Lua will not index a table constructor: `{...}["a"]` does
            // not parse, and the printer reads every field straight off the value.
            format!("({{{}}})", parts.join(", "))
        }

        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(expr).collect();
            format!("{{{}}}", parts.join(", "))
        }
        Kind::Call { func, arg } => format!("{}({})", user(func), expr(arg)),
        // Parenthesised because there is more than one operator, and Lua's precedence is not
        // toylang's to rely on.
        Kind::Concat(l, r) => format!("({} .. {})", expr(l), expr(r)),
        Kind::Arith { op, lhs, rhs } => match op {
            BinOp::Div => format!("tl_div({}, {})", expr(lhs), expr(rhs)),
            BinOp::Rem => format!("tl_rem({}, {})", expr(lhs), expr(rhs)),
            BinOp::Add => format!("tl_i32({} + {})", expr(lhs), expr(rhs)),
            BinOp::Sub => format!("tl_i32({} - {})", expr(lhs), expr(rhs)),
            BinOp::Mul => format!("tl_i32({} * {})", expr(lhs), expr(rhs)),
            other => unreachable!("{other} is not arithmetic"),
        },
        Kind::Cond { cond, then, otherwise } => format!(
            "(function() if {} then return {} else return {} end end)()",
            expr(cond),
            expr(then),
            expr(otherwise)
        ),
        Kind::Builtin { which, arg } => match which {
            Builtin::IntToStr => format!("tostring({})", expr(arg)),
            Builtin::Range => format!("tl_range({})", expr(arg)),
            Builtin::JsonLines => {
                let elem = arg.ty.elem().expect("checked to be a Vec");
                let e = "e0".to_string();
                format!(
                    "tl_jsonlines({}, function({e}) return {} end)",
                    expr(arg),
                    show(elem, &e, 1)
                )
            }
            Builtin::Collect => "tl_collect_lines()".to_string(),
            Builtin::Extent => format!("#{}", expr(arg)),
            Builtin::Tail => format!("tl_tail({})", expr(arg)),
            Builtin::Concat => format!("tl_vec_concat({})", expr(arg)),
        },
        Kind::Compare { op, lhs, rhs } => {
            format!("({} {} {})", expr(lhs), lua_op(*op), expr(rhs))
        }
        // Lua has no expression-level `let`, so the binding becomes a call.
        Kind::Bind { local: id, value, body } => {
            format!("(function({}) return {} end)({})", local(*id), expr(body), expr(value))
        }
        Kind::Map { source, param, body } => format!(
            "tl_map({}, function({}) return {} end)",
            expr(source),
            local(*param),
            expr(body)
        ),
        Kind::Select { source, param, pred } => format!(
            "tl_select({}, function({}) return {} end)",
            expr(source),
            local(*param),
            expr(pred)
        ),
        // The depth comes from the type on the node below, so it cannot disagree with it, and
        // the emitted helper is told the answer rather than inspecting the value for it.
        Kind::Unwrap { base } => {
            format!("tl_unwrap({}, {})", expr(base), tir::vec_depth(&base.ty))
        }
        // A Lua table of records is a table of tables, so collapsing needs no gather here;
        // `elem_is_record` matters only where the columns are stored apart.
        Kind::Index { base, index, depth, .. } => {
            format!("tl_at({}, {}, {})", expr(base), expr(index), depth)
        }
        Kind::Field { base, name } => {
            let depth = tir::vec_depth(&base.ty);
            if depth == 0 {
                format!("{}[{}]", expr(base), lua_string(name))
            } else {
                format!("tl_field({}, {}, {})", expr(base), lua_string(name), depth)
            }
        }
    }
}

fn lua_op(op: BinOp) -> &'static str {
    match op {
        BinOp::Eq => "==",
        BinOp::Ne => "~=",
        BinOp::Lt => "<",
        BinOp::Le => "<=",
        BinOp::Gt => ">",
        BinOp::Ge => ">=",
        other => unreachable!("{other} is not a comparison"),
    }
}

fn lua_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            // Lua reads \ddd as a byte, so anything above ASCII has to go through as its
            // UTF-8 bytes rather than as a codepoint escape.
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\{:03}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
