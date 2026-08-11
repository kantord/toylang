pub mod ast;
pub mod check;
pub mod emit_js;
pub mod emit_lua;
pub mod error;
pub mod input;
pub mod lex;
pub mod parse;
pub mod tir;
pub mod ty;

use std::cell::RefCell;
use std::io::Write;
use std::rc::Rc;

use error::Error;
use tir::Program;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Lua,
    Js,
}

impl Backend {
    pub const ALL: [Backend; 2] = [Backend::Lua, Backend::Js];

    pub fn name(self) -> &'static str {
        match self {
            Backend::Lua => "lua",
            Backend::Js => "js",
        }
    }

    pub fn emit(self, program: &Program) -> String {
        match self {
            Backend::Lua => emit_lua::emit(program),
            Backend::Js => emit_js::emit(program),
        }
    }
}

pub fn compile(src: &str) -> Result<Program, Error> {
    let tokens = lex::lex(src)?;
    let file = parse::parse(&tokens)?;
    check::check(&file)
}

pub fn run(src: &str) -> Result<String, Box<dyn std::error::Error>> {
    run_on(src, None, Backend::Lua)
}

pub fn run_with_input(
    src: &str,
    stdin: Option<&str>,
) -> Result<String, Box<dyn std::error::Error>> {
    run_on(src, stdin, Backend::Lua)
}

/// Compile and run, capturing what the program printed.
///
/// Capturing rather than streaming keeps the tests to one call. It also means output is held in
/// memory, which is fine while every program has statically known extent and will not be once
/// streaming input exists.
pub fn run_on(
    src: &str,
    stdin: Option<&str>,
    backend: Backend,
) -> Result<String, Box<dyn std::error::Error>> {
    let program = compile(src)?;

    // The input is checked against the declared type once, here, rather than by each backend.
    // What a backend receives has already been parsed, so no backend re-decides what is valid.
    let value = match (&program.input, stdin) {
        (Some(ty), Some(text)) => {
            let value: serde_json::Value = serde_json::from_str(text)?;
            input::validate(&value, ty, "input")?;
            Some(value)
        }
        (Some(ty), None) => return Err(format!("this program reads input, of type {ty}").into()),
        (None, _) => None,
    };

    let source = backend.emit(&program);
    match backend {
        Backend::Lua => run_lua(&source, value.as_ref()),
        Backend::Js => run_node(&source, value.as_ref()),
    }
}

fn run_lua(
    source: &str,
    value: Option<&serde_json::Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    let lua = mlua::Lua::new();
    if let Some(value) = value {
        lua.globals().set(emit_lua::INPUT, input::to_lua(&lua, value)?)?;
    }

    let captured = Rc::new(RefCell::new(String::new()));
    let sink = Rc::clone(&captured);
    let print = lua.create_function(move |_, s: String| {
        sink.borrow_mut().push_str(&s);
        sink.borrow_mut().push('\n');
        Ok(())
    })?;
    lua.globals().set("print", print)?;

    lua.load(source).exec()?;

    let out = captured.borrow().clone();
    Ok(out)
}

/// Runs through `node`, which must be present. A missing toolchain is an error rather than a
/// quietly skipped backend: a report that says two backends agreed when only one ran is worse
/// than no report.
fn run_node(
    source: &str,
    value: Option<&serde_json::Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("program.js");
    std::fs::write(&path, source)?;

    let mut child = std::process::Command::new("node")
        .arg(&path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run `node`: {e}"))?;

    // Written even when empty, so a program that does not read input still sees stdin close.
    let text = value.map(|v| v.to_string()).unwrap_or_default();
    child.stdin.take().expect("piped").write_all(text.as_bytes())?;

    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(format!("node failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

impl std::error::Error for Error {}
