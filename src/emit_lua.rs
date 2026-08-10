use crate::ir::{Ir, Program};

pub fn emit(program: &Program) -> String {
    match &program.body {
        Ir::ConstStr(s) => format!("print({})\n", lua_string(s)),
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
