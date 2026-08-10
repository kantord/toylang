use crate::ir::{Ir, Program};

pub fn emit(program: &Program) -> String {
    let mut out = String::new();

    // All names are declared before any body, because the checker collects signatures before
    // checking bodies and so accepts a call to a function defined further down. Emitting
    // `local function` in source order would leave that call resolving to a nil global.
    if !program.funcs.is_empty() {
        let names: Vec<String> = program.funcs.iter().map(|f| name(&f.name)).collect();
        out.push_str(&format!("local {}\n", names.join(", ")));
    }

    for f in &program.funcs {
        out.push_str(&format!(
            "function {}({})\n  return {}\nend\n",
            name(&f.name),
            name(&f.param),
            expr(&f.body)
        ));
    }

    out.push_str(&format!("print({})\n", expr(&program.body)));
    out
}

/// Every toylang name is prefixed, because the target's namespace is not ours. A program with a
/// function called `print` or `end` would otherwise emit Lua that shadows the output function or
/// does not parse.
fn name(n: &str) -> String {
    format!("v_{n}")
}

fn expr(ir: &Ir) -> String {
    match ir {
        Ir::ConstStr(s) => lua_string(s),
        Ir::ConstInt(n) => n.to_string(),
        Ir::Var(n) => name(n),
        Ir::Call { func, arg } => format!("{}({})", name(func), expr(arg)),
        // No parentheses: `..` is the only operator, and it is associative over strings, so
        // nesting cannot change the result. A second operator makes them necessary.
        Ir::Concat(l, r) => format!("{} .. {}", expr(l), expr(r)),
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
