use crate::ir::{Ir, Program};
use crate::ty::Type;

const SELECT_HELPER: &str = "\
local function tl_select(src, pred)
  local out = {}
  for i = 1, #src do
    if pred(src[i]) then out[#out + 1] = src[i] end
  end
  return out
end
";

const SHOW_HELPER: &str = r#"local function tl_quote(s)
  return '"' .. s:gsub('[%c"\\]', function(c)
    if c == '"' then return '\\"' end
    if c == '\\' then return '\\\\' end
    if c == '\n' then return '\\n' end
    if c == '\r' then return '\\r' end
    if c == '\t' then return '\\t' end
    return string.format('\\u%04x', c:byte())
  end) .. '"'
end
local function tl_show(v)
  local t = type(v)
  if t == "table" then
    local parts = {}
    for i = 1, #v do parts[i] = tl_show(v[i]) end
    return "[" .. table.concat(parts, ",") .. "]"
  elseif t == "string" then
    return tl_quote(v)
  end
  return tostring(v)
end
"#;

pub fn emit(program: &Program, ty: &Type) -> String {
    let mut out = String::new();

    if uses_select(program) {
        out.push_str(SELECT_HELPER);
    }
    // A top-level Str prints raw, the way jq's -r does; anything else prints as JSON. So a
    // string inside a Vec is quoted while a bare string is not.
    let structured = *ty != Type::Str;
    if structured {
        out.push_str(SHOW_HELPER);
    }

    // All names are declared before any body, because the checker collects signatures before
    // checking bodies and so accepts a call to a function defined further down. Emitting
    // `local function` in source order would leave that call resolving to a nil global.
    if !program.funcs.is_empty() {
        let names: Vec<&str> = program.funcs.iter().map(|f| f.name.as_str()).collect();
        out.push_str(&format!("local {}\n", names.join(", ")));
    }

    for f in &program.funcs {
        out.push_str(&format!(
            "function {}({})\n  return {}\nend\n",
            f.name,
            f.param,
            expr(&f.body)
        ));
    }

    let body = expr(&program.body);
    if structured {
        out.push_str(&format!("print(tl_show({body}))\n"));
    } else {
        out.push_str(&format!("print({body})\n"));
    }
    out
}

fn uses_select(program: &Program) -> bool {
    fn walk(ir: &Ir) -> bool {
        match ir {
            Ir::Select { .. } => true,
            Ir::ConstStr(_) | Ir::ConstInt(_) | Ir::Local(_) => false,
            Ir::VecLit(items) => items.iter().any(walk),
            Ir::Call { arg, .. } => walk(arg),
            Ir::Concat(l, r) | Ir::Compare { lhs: l, rhs: r, .. } => walk(l) || walk(r),
            Ir::Bind { value, body, .. } => walk(value) || walk(body),
        }
    }
    program.funcs.iter().any(|f| walk(&f.body)) || walk(&program.body)
}

fn expr(ir: &Ir) -> String {
    match ir {
        Ir::ConstStr(s) => lua_string(s),
        Ir::ConstInt(n) => n.to_string(),
        Ir::Local(n) => n.clone(),
        Ir::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(expr).collect();
            format!("{{{}}}", parts.join(", "))
        }
        Ir::Call { func, arg } => format!("{}({})", func, expr(arg)),
        // Parenthesised because there is now more than one operator, and Lua's precedence is
        // not toylang's to rely on.
        Ir::Concat(l, r) => format!("({} .. {})", expr(l), expr(r)),
        Ir::Compare { op, lhs, rhs } => format!("({} {} {})", expr(lhs), op, expr(rhs)),
        // Lua has no expression-level `let`, so the binding becomes a call.
        Ir::Bind { name, value, body } => {
            format!("(function({}) return {} end)({})", name, expr(body), expr(value))
        }
        Ir::Select { source, param, pred } => {
            format!("tl_select({}, function({}) return {} end)", expr(source), param, expr(pred))
        }
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
