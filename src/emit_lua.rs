use crate::ast::BinOp;
use crate::tir::{self, Kind, LocalId, Program, Tir};
use crate::ty::Type;

/// The global the input value is bound to before the chunk runs. Unspellable in source, since
/// every source name is prefixed.
pub const INPUT: &str = "t_input";

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
    if structured && needs_quote(&program.body.ty) {
        out.push_str(QUOTE_HELPER);
    }
    if structured && contains_vec(&program.body.ty) {
        out.push_str(JOIN_HELPER);
    }
    if used.index || used.unwrap || contains_opt(&program.body.ty) {
        out.push_str(OPT_HELPER);
    }
    if used.unwrap {
        out.push_str(UNWRAP_HELPER);
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

    let body = expr(&program.body);
    if structured {
        out.push_str(&format!("print({})\n", show(&program.body.ty, &body, 0)));
    } else {
        out.push_str(&format!("print({body})\n"));
    }
    out
}

/// The printer is built from the type rather than by inspecting the value. A Lua table cannot
/// say whether it is an array or a record -- an empty one is indistinguishable either way -- so
/// asking it was always going to disagree with a backend that knows. See emit_js for the same
/// function, and step 4 onwards, where a native target has nothing to ask.
fn show(ty: &Type, value: &str, depth: usize) -> String {
    match ty {
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
                // Depth zero is a plain index and needs no helper.
                used.field |= tir::vec_depth(&base.ty) > 0;
                walk(base, used);
            }
            Kind::IntToStr(n) => walk(n, used),
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
        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(expr).collect();
            format!("{{{}}}", parts.join(", "))
        }
        Kind::Call { func, arg } => format!("{}({})", user(func), expr(arg)),
        // Parenthesised because there is more than one operator, and Lua's precedence is not
        // toylang's to rely on.
        Kind::Concat(l, r) => format!("({} .. {})", expr(l), expr(r)),
        Kind::Compare { op, lhs, rhs } => {
            format!("({} {} {})", expr(lhs), lua_op(*op), expr(rhs))
        }
        // Lua has no expression-level `let`, so the binding becomes a call.
        Kind::Bind { local: id, value, body } => {
            format!("(function({}) return {} end)({})", local(*id), expr(body), expr(value))
        }
        Kind::Select { source, param, pred } => format!(
            "tl_select({}, function({}) return {} end)",
            expr(source),
            local(*param),
            expr(pred)
        ),
        // The depth comes from the type on the node below, so it cannot disagree with it, and
        // the emitted helper is told the answer rather than inspecting the value for it.
        Kind::IntToStr(n) => format!("tostring({})", expr(n)),
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
        BinOp::Add => unreachable!("Add is emitted as Concat"),
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
