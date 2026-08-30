use crate::ast::BinOp;
use crate::tir::{self, Builtin, Kind, LocalId, Program, Tir};
use crate::ty::{self, Enums, Type};

/// The global the input value is bound to before the chunk runs. Unspellable in source, since
/// every source name is prefixed.
pub const INPUT: &str = "t_input";
/// The global every remaining stdin line, already parsed, is bound to before the chunk runs.
pub const INPUTS: &str = "t_inputs";
/// The function `run_lua` injects in place of `INPUTS` for a fused, live-streamable program:
/// called with no arguments, it returns the next `inputs` record already parsed and converted,
/// or `nil` at EOF. See `tir::fusion` and this file's `fused_main`.
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
-- An Opt is its enum's own runtime shape (ADR 0009): `{some = v}` present, \"none\" absent.
-- Tagged, so two levels of absence stay two values; only the printer flattens to null. A
-- table rather than nil also keeps `#` honest for a Vec of Opt.
local function tl_at(v, i, depth)
  if depth > 0 then
    local out = {}
    for k = 1, #v do out[k] = tl_at(v[k], i, depth - 1) end
    return out
  end
  local n = #v
  if i < 0 then i = n + i end
  if i < 0 or i >= n then return \"none\" end
  return { some = v[i + 1] }
end
";

const TAIL_HELPER: &str = "\
local function tl_tail(v)
  if #v == 0 then return \"none\" end
  local out = {}
  for i = 2, #v do out[i - 1] = v[i] end
  return { some = out }
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

// `table.sort`'s default comparator is Lua's own `<`, which already agrees with every other
// backend's ordering on both numbers and (byte-wise, so codepoint-wise for valid UTF-8) strings,
// so nothing here has to branch on the element type.
const SORT_HELPER: &str = "\
local function tl_sort(v)
  local out = {}
  for i = 1, #v do out[i] = v[i] end
  table.sort(out)
  return out
end
";

const REVERSE_HELPER: &str = "\
local function tl_reverse(v)
  local out = {}
  local n = #v
  for i = 1, n do out[i] = v[n - i + 1] end
  return out
end
";

const UNWRAP_HELPER: &str = r#"local function tl_unwrap(v, depth)
  if depth > 0 then
    local out = {}
    for k = 1, #v do out[k] = tl_unwrap(v[k], depth - 1) end
    return out
  end
  if v == "none" then error("toylang: unwrapped a value that is not there", 0) end
  return v.some
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

// Lua's integers already are 64-bit and wrap (kantord/toylang#83), so an Int64's `+`, `-` and
// `*` need nothing at all -- the one backend where the wider type is cheaper than the narrow
// one. Only division needs spelling: `//` floors where toylang truncates, and `math.fmod` is
// already truncated, so the same fmod-then-divide `tl_div` uses works with the `tl_i32` taken
// off. `MIN // -1` wraps back to `MIN` on its own, Lua's documented overflow rule.
const ARITH64_HELPER: &str = r#"local function tl_div64(a, b)
  if b == 0 then error("toylang: divided by zero", 0) end
  return (a - math.fmod(a, b)) // b
end
local function tl_rem64(a, b)
  if b == 0 then error("toylang: divided by zero", 0) end
  return math.fmod(a, b)
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

// Lua strings are byte arrays with no Unicode awareness, so decoding by codepoint (rather than
// by byte) has to be hand-rolled, the same as the native runtime's tl_chars does in C.
const CHARS_HELPER: &str = r#"local function tl_chars(s)
  local out = {}
  local i = 1
  local n = #s
  while i <= n do
    local b0 = s:byte(i)
    local cp, extra
    if b0 < 0x80 then cp, extra = b0, 0
    elseif b0 & 0xE0 == 0xC0 then cp, extra = b0 & 0x1F, 1
    elseif b0 & 0xF0 == 0xE0 then cp, extra = b0 & 0x0F, 2
    else cp, extra = b0 & 0x07, 3
    end
    i = i + 1
    for _ = 1, extra do
      cp = (cp << 6) | (s:byte(i) & 0x3F)
      i = i + 1
    end
    out[#out + 1] = cp
  end
  return out
end
"#;

pub fn emit(program: &Program) -> String {
    let enums = &program.enums;
    let mut out = String::new();

    let used = used_helpers(program);
    // A top-level Str prints raw, the way jq's -r does; anything else prints as JSON. So a
    // string inside a Vec is quoted while a bare string is not.
    let structured = program.body.ty != Type::Str;
    let quote = (structured && needs_quote(&program.body.ty)) || used.jsonlines;
    let join = (structured && contains_vec(enums, &program.body.ty)) || used.jsonlines;
    for (on, text) in [
        (used.select, SELECT_HELPER),
        (used.field, FIELD_HELPER),
        (quote, QUOTE_HELPER),
        (join, JOIN_HELPER),
        (used.index, OPT_HELPER),
        (used.unwrap, UNWRAP_HELPER),
        (used.tail, TAIL_HELPER),
        (used.concat, VEC_CONCAT_HELPER),
        (used.sort, SORT_HELPER),
        (used.reverse, REVERSE_HELPER),
        (used.arith, ARITH_HELPER),
        (used.arith64, ARITH64_HELPER),
        (used.map, MAP_HELPER),
        (used.range, RANGE_HELPER),
        (used.collect, COLLECT_HELPER),
        (used.jsonlines, JSONLINES_HELPER),
        (used.chars, CHARS_HELPER),
    ] {
        if on {
            out.push_str(text);
        }
    }

    out.push_str(&printers(program));

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
            f.param.as_deref().map_or_else(String::new, user),
            expr(enums, &f.body)
        ));
    }

    if let Some(fusion) = tir::fusion(program) {
        out.push_str(&fused_main(program, &fusion));
    } else {
        let body = expr(enums, &program.body);
        if structured {
            out.push_str(&format!(
                "print({})\n",
                show(enums, &program.body.ty, &body, 0)
            ));
        } else {
            out.push_str(&format!("print({body})\n"));
        }
    }
    out
}

/// A stream-typed `jsonlines` program, compiled as a loop over one entry at a time: for an
/// `inputs` source, a call to `NEXT_INPUT` for one already-parsed record (see that constant's
/// doc comment for why the parsing itself is not written here); for `lines`, `io.lines()`
/// itself, exactly as the eager collect helper reads it.
fn fused_main(program: &Program, fusion: &tir::Fusion) -> String {
    let enums = &program.enums;
    let mut out = String::new();
    let (mut current, mut current_ty) = match fusion.source {
        tir::Source::Inputs => {
            out.push_str("while true do\n");
            out.push_str(&format!("  local t_line = {NEXT_INPUT}()\n"));
            out.push_str("  if t_line == nil then break end\n");
            let elem = program
                .inputs
                .as_ref()
                .expect("an inputs source recorded its element");
            ("t_line".to_string(), elem.clone())
        }
        tir::Source::Lines => {
            out.push_str("for t_line in io.lines() do\n");
            ("t_line".to_string(), Type::Str)
        }
    };
    for stage in &fusion.stages {
        match stage {
            tir::Stage::Map { param, body } => {
                out.push_str(&format!("  local {} = {}\n", local(*param), current));
                current = expr(enums, body);
                current_ty = body.ty.clone();
            }
            tir::Stage::Select { param, pred } => {
                out.push_str(&format!("  local {} = {}\n", local(*param), current));
                out.push_str(&format!(
                    "  if not ({}) then goto tl_continue end\n",
                    expr(enums, pred)
                ));
                current = local(*param);
            }
        }
    }
    // Always the JSON rendering, never the top-level raw-Str rule: each line is one JSON value
    // (that is what jsonlines promises), so a Str element prints quoted here exactly as the
    // eager path's per-element `show` prints it. Only whole-program results print raw.
    let printed = show(enums, &current_ty, &current, 0);
    out.push_str(&format!("  print({printed})\n"));
    out.push_str("  ::tl_continue::\n");
    out.push_str("end\n");
    out
}

/// The printer is built from the type rather than by inspecting the value. A Lua table cannot
/// say whether it is an array or a record -- an empty one is indistinguishable either way -- so
/// asking it was always going to disagree with a backend that knows. See emit_js for the same
/// function, and step 4 onwards, where a native target has nothing to ask.
fn show(enums: &Enums, ty: &Type, value: &str, depth: usize) -> String {
    match ty {
        Type::Param(_) => unreachable!("params are substituted before emit"),
        // The checker refuses a program whose result contains a stream, since there is nothing to
        // print: a stream has no value, only a promise that collect can redeem.
        Type::Stream(_) => unreachable!("a stream cannot reach the printer"),
        Type::Char => unreachable!("Char cannot reach the printer, refused by the checker"),
        Type::Str => format!("tl_quote({value})"),
        Type::Int | Type::Int64 | Type::Bool => format!("tostring({value})"),
        Type::Vec(elem) => {
            let e = format!("e{depth}");
            format!(
                "tl_join({value}, function({e}) return {} end)",
                show(enums, elem, &e, depth + 1)
            )
        }
        Type::Enum { .. } if ty.as_opt().is_some() => {
            let inner = ty.as_opt().expect("guarded");
            let v = format!("o{depth}");
            format!(
                "(function({v}) if {v} == \"none\" then return \"null\" else return {} end end)({value})",
                show(enums, inner, &format!("{v}.some"), depth + 1)
            )
        }
        // A recursive enum prints through a function of its own (`printers`), because expanding
        // one here has no bottom: its payload leads back to the same type.
        Type::Enum { .. } if ty::is_recursive(enums, ty) => format!("{}({value})", ty.show_fn()),
        Type::Enum { .. } => show_enum(enums, ty, value, depth),
        Type::Record(fields) => {
            // `..` needs an operand on each side, so a record with no fields cannot be
            // built by joining nothing. The other backends survive this by construction.
            if fields.is_empty() {
                return "\"{}\"".to_string();
            }
            // The type's field list is declaration order, so this prints as declared. Field
            // names are identifiers, so the JSON key needs no escaping and is one literal.
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, fty)| {
                    let read = format!("{value}[{}]", lua_string(name));
                    let key = lua_string(&format!("\"{name}\":"));
                    format!("{key} .. {}", show(enums, fty, &read, depth + 1))
                })
                .collect();
            format!("(\"{{\" .. {} .. \"}}\")", parts.join(" .. \",\" .. "))
        }
    }
}

/// The printer for one enum, inline. One type, two runtime shapes (ADR 0009): a unit variant is
/// a bare string, a payload variant a single-key table, so this is the one printer that has to
/// look before it renders. Only the shape is inspected; which payload follows which key is the
/// type's.
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
        body.push_str(&format!(
            "if type({n}) == \"string\" then return tl_quote({n}) end "
        ));
    }
    for (i, (vname, pty)) in payloads.iter().enumerate() {
        let pty = pty.as_ref().expect("filtered to payload variants");
        let read = format!("{n}[{}]", lua_string(vname));
        let wrapped = format!(
            "({} .. {} .. \"}}\")",
            lua_string(&format!("{{\"{vname}\":")),
            show(enums, pty, &read, depth + 1)
        );
        if i + 1 < payloads.len() {
            body.push_str(&format!("if {read} ~= nil then return {wrapped} end "));
        } else {
            // The last payload variant needs no test: the type says nothing else is left.
            body.push_str(&format!("return {wrapped} "));
        }
    }
    format!("(function({n}) {body}end)({value})")
}


/// A named printer for every recursive enum the program prints. The call in `show` above is
/// what a nested occurrence renders as, so the recursion in the type becomes recursion in the
/// emitted function rather than in this compiler (kantord/toylang#94).
fn printers(program: &Program) -> String {
    let mut out = String::new();
    for ty in tir::printed_recursive_enums(program) {
        out.push_str(&format!(
            "function {}(v)\n  return {}\nend\n",
            ty.show_fn(),
            show_enum(&program.enums, &ty, "v", 0)
        ));
    }
    out
}

fn needs_quote(ty: &Type) -> bool {
    match ty {
        Type::Str => true,
        Type::Vec(elem) => needs_quote(elem),
        Type::Record(_) => true,
        // A unit variant prints as a quoted string, and a payload wrapper is a record.
        Type::Enum { .. } => true,
        _ => false,
    }
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
    arith64: bool,
    map: bool,
    range: bool,
    collect: bool,
    jsonlines: bool,
    tail: bool,
    concat: bool,
    chars: bool,
    sort: bool,
    reverse: bool,
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
            Kind::OptMap { source, body, .. } => {
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
                used.jsonlines |= *which == Builtin::JsonLines;
                used.tail |= *which == Builtin::Tail;
                used.concat |= *which == Builtin::Concat;
                used.chars |= *which == Builtin::Chars;
                used.sort |= *which == Builtin::Sort;
                used.reverse |= *which == Builtin::Reverse;
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
            Kind::Arith { op, lhs, rhs } => {
                if t.ty == Type::Int64 {
                    used.arith64 |= matches!(op, BinOp::Div | BinOp::Rem);
                } else {
                    used.arith = true;
                }
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
        format!("return {{some = {body}}} ")
    } else {
        format!("return {body} ")
    }
}

fn expr(enums: &Enums, t: &Tir) -> String {
    match &t.kind {
        Kind::Str(s) => lua_string(s),
        Kind::Int(n) => n.to_string(),
        Kind::Var(name) => user(name),
        Kind::Local(id) => local(*id),
        Kind::Input => INPUT.to_string(),
        Kind::Inputs => INPUTS.to_string(),
        // The stream, materialized eagerly: whatever consumes it -- `collect`, a mapper --
        // works on the table of its entries. Fusion is what will remove this materialization.
        Kind::Lines => "tl_collect_lines()".to_string(),
        // A record is a table keyed by field name, which is what field access reads.
        Kind::RecordLit { fields } => {
            let parts: Vec<String> = fields
                .iter()
                .map(|(name, value)| format!("[{}] = {}", lua_string(name), expr(enums, value)))
                .collect();
            // Parenthesised because Lua will not index a table constructor: `{...}["a"]` does
            // not parse, and the printer reads every field straight off the value.
            format!("({{{}}})", parts.join(", "))
        }

        // The value is its JSON shape: a unit variant is the variant-name string, a payload
        // variant the single-key table a record already is.
        Kind::EnumLit { variant, payload } => match payload {
            None => lua_string(variant),
            Some(p) => format!("({{[{}] = {}}})", lua_string(variant), expr(enums, p)),
        },

        Kind::VecLit(items) => {
            let parts: Vec<String> = items.iter().map(|i| expr(enums, i)).collect();
            format!("{{{}}}", parts.join(", "))
        }
        Kind::Call { func, arg } => format!(
            "{}({})",
            user(func),
            arg.as_deref().map_or_else(String::new, |a| expr(enums, a))
        ),
        // Parenthesised because there is more than one operator, and Lua's precedence is not
        // toylang's to rely on.
        Kind::Concat(l, r) => format!("({} .. {})", expr(enums, l), expr(enums, r)),
        Kind::Arith { op, lhs, rhs } => arith(&t.ty, *op, expr(enums, lhs), expr(enums, rhs)),
        Kind::Cond {
            cond,
            then,
            otherwise,
        } => format!(
            "(function() if {} then return {} else return {} end end)()",
            expr(enums, cond),
            expr(enums, then),
            expr(enums, otherwise)
        ),
        Kind::Builtin { which, arg } => match which {
            Builtin::IntToStr => format!("tostring({})", expr(enums, arg)),
            // Lua's integers are 64-bit already; an Int just lives in the low half.
            Builtin::IntToI64 => expr(enums, arg),
            Builtin::Chars => format!("tl_chars({})", expr(enums, arg)),
            Builtin::Range => format!("tl_range({})", expr(enums, arg)),
            Builtin::JsonLines => {
                let elem = tir::runtime_elem(&arg.ty).expect("checked to be a Vec or a stream");
                let e = "e0".to_string();
                format!(
                    "tl_jsonlines({}, function({e}) return {} end)",
                    expr(enums, arg),
                    show(enums, elem, &e, 1)
                )
            }
            // The source already materialized, so the exit has nothing left to do.
            Builtin::Collect => expr(enums, arg),
            Builtin::Extent => format!("#{}", expr(enums, arg)),
            Builtin::Tail => format!("tl_tail({})", expr(enums, arg)),
            Builtin::Concat => format!("tl_vec_concat({})", expr(enums, arg)),
            Builtin::Sort => format!("tl_sort({})", expr(enums, arg)),
            Builtin::Reverse => format!("tl_reverse({})", expr(enums, arg)),
            // The names come from the checked type, not the table value, so `arg` runs as the
            // function literal's ignored parameter -- the same IIFE shape `Bind` uses -- purely
            // for whatever else it does.
            Builtin::Fields => {
                let Type::Record(fields) = &arg.ty else {
                    unreachable!("checked to be a record")
                };
                let names: Vec<String> = fields.iter().map(|(n, _)| lua_string(n)).collect();
                format!(
                    "(function(_) return {{{}}} end)({})",
                    names.join(", "),
                    expr(enums, arg)
                )
            }
        },
        Kind::Compare { op, lhs, rhs } => {
            format!(
                "({} {} {})",
                expr(enums, lhs),
                lua_op(*op),
                expr(enums, rhs)
            )
        }
        // Lua has no expression-level `let`, so the binding becomes a call.
        Kind::Bind {
            local: id,
            value,
            body,
        } => {
            format!(
                "(function({}) return {} end)({})",
                local(*id),
                expr(enums, body),
                expr(enums, value)
            )
        }
        Kind::Map {
            source,
            param,
            body,
        } => format!(
            "tl_map({}, function({}) return {} end)",
            expr(enums, source),
            local(*param),
            expr(enums, body)
        ),
        Kind::Select {
            source,
            param,
            pred,
        } => format!(
            "tl_select({}, function({}) return {} end)",
            expr(enums, source),
            local(*param),
            expr(enums, pred)
        ),
        // The depth comes from the type on the node below, so it cannot disagree with it, and
        // the emitted helper is told the answer rather than inspecting the value for it.
        Kind::Unwrap { base } => {
            format!(
                "tl_unwrap({}, {})",
                expr(enums, base),
                tir::vec_depth(&base.ty)
            )
        }
        // Opt's reorder pass (kantord/toylang#66): the same `== "none"`/`.some` shape the
        // printer and Match already read, generalised to rebuild the table instead.
        Kind::OptMap {
            source,
            param,
            body,
        } => format!(
            "(function(__opt) if __opt == \"none\" then return \"none\" else local {} = __opt.some return {{some = {}}} end end)({})",
            local(*param),
            expr(enums, body),
            expr(enums, source)
        ),
        // A Lua table of records is a table of tables, so collapsing needs no gather here;
        // `elem_is_record` matters only where the columns are stored apart.
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
                format!("{}[{}]", expr(enums, base), lua_string(name))
            } else {
                format!(
                    "tl_field({}, {}, {})",
                    expr(enums, base),
                    lua_string(name),
                    depth
                )
            }
        }
        // A chain of tests over the subject (a plain local, so re-reading it is free): string
        // equality for a unit variant, key presence for a payload variant, the guard's own
        // Bool for a guard arm. A total chain's last arm needs no test, the checker having
        // proved nothing else can reach it; a partial chain tests every arm and falls through
        // to `tl_none`, the absent Opt.
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
                        "local {} = {subj}[{}] ",
                        local(pid),
                        lua_string(variant)
                    ));
                }
                run.push_str(&arm_return(expr(enums, &arm.body), *partial));
                let test = match (&arm.variant, &arm.guard) {
                    (Some(v), _) if arm.payload.is_some() => {
                        Some(format!("{subj}[{}] ~= nil", lua_string(v)))
                    }
                    (Some(v), _) => Some(format!("{subj} == {}", lua_string(v))),
                    (None, Some(g)) => Some(expr(enums, g)),
                    (None, None) => None,
                };
                match test {
                    Some(test) if *partial || i + 1 < arms.len() => {
                        body.push_str(&format!("if {test} then {run}end "));
                    }
                    _ => body.push_str(&run),
                }
            }
            if *partial {
                body.push_str("return \"none\" ");
            }
            format!("(function() {body}end)()")
        }
    }
}

/// One arithmetic expression at the width the node's type names. At 64 bits Lua's own
/// integers are the semantics (kantord/toylang#83): `+`, `-` and `*` wrap natively, so only
/// division and remainder go through a helper.
fn arith(ty: &Type, op: BinOp, l: String, r: String) -> String {
    if *ty == Type::Int64 {
        match op {
            BinOp::Div => format!("tl_div64({l}, {r})"),
            BinOp::Rem => format!("tl_rem64({l}, {r})"),
            BinOp::Add => format!("({l} + {r})"),
            BinOp::Sub => format!("({l} - {r})"),
            BinOp::Mul => format!("({l} * {r})"),
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
