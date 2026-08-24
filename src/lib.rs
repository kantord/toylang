pub mod ast;
pub mod check;
pub mod emit_go;
pub mod emit_jq;
pub mod emit_js;
pub mod emit_llvm;
pub mod emit_lua;
pub mod emit_py;
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
    Native,
    Jq,
    Go,
    Py,
}

impl Backend {
    /// The backends that must agree on every corpus program. Native joined once it could
    /// compile the whole language; until then it was kept out rather than allowed to turn the
    /// harness permanently red, and tests/backend_llvm.rs tracked what it was missing.
    pub const ALL: [Backend; 6] =
        [Backend::Lua, Backend::Js, Backend::Native, Backend::Jq, Backend::Go, Backend::Py];

    pub fn name(self) -> &'static str {
        match self {
            Backend::Lua => "lua",
            Backend::Js => "js",
            Backend::Native => "native",
            Backend::Jq => "jq",
            Backend::Go => "go",
            Backend::Py => "py",
        }
    }

    pub fn emit(self, program: &Program) -> Result<String, String> {
        Ok(match self {
            Backend::Lua => emit_lua::emit(program),
            Backend::Js => emit_js::emit(program),
            Backend::Native => return emit_llvm::to_ir(program),
            Backend::Jq => emit_jq::emit(program),
            Backend::Go => emit_go::emit(program),
            Backend::Py => emit_py::emit(program),
        })
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

    match backend {
        Backend::Lua => run_lua(&emit_lua::emit(&program), value.as_ref()),
        Backend::Js => run_node(&emit_js::emit(&program), value.as_ref()),
        Backend::Jq => run_jq(
            &emit_jq::emit(&program),
            value.as_ref(),
            program.body.ty == ty::Type::Str,
        ),
        Backend::Go => run_go(&emit_go::emit(&program), value.as_ref()),
        Backend::Py => run_py(&emit_py::emit(&program), value.as_ref()),
        Backend::Native => {
            let dir = tempfile::tempdir()?;
            let exe = dir.path().join("program");
            link(&program, &exe)?;
            run_binary(&exe, value.as_ref())
        }
    }
}

/// Compile to a native executable at `out`.
///
/// LLVM produces an object file, which is not a program, so this shells out to `cc` for the
/// link. That is a toolchain requirement the Lua backend does not have, since mlua vendors its
/// interpreter.
pub fn link(program: &Program, out: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let object = dir.path().join("program.o");
    emit_llvm::compile_to_object(program, &object)?;

    // The runtime is compiled alongside rather than shipped as a library, which keeps the build
    // to one `cc` call and means there is nothing to install.
    let runtime = dir.path().join("toylang.c");
    std::fs::write(&runtime, emit_llvm::RUNTIME_C)?;

    let status = std::process::Command::new("cc")
        .arg(&object)
        .arg(&runtime)
        .arg("-o")
        .arg(out)
        .status()
        .map_err(|e| format!("could not run `cc`: {e}"))?;
    if !status.success() {
        return Err(format!("cc failed to link: {status}").into());
    }
    Ok(())
}

fn run_binary(
    exe: &std::path::Path,
    value: Option<&serde_json::Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut child = std::process::Command::new(exe)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let text = value.map(|v| v.to_string()).unwrap_or_default();
    child.stdin.take().expect("piped").write_all(text.as_bytes())?;

    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(format!(
            "the compiled program failed: {}",
            String::from_utf8_lossy(&out.stderr)
        )
        .into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

/// jq puts stdin in `.`, so a program that reads no input runs with `-n` rather than waiting on
/// a terminal.
fn run_jq(
    source: &str,
    value: Option<&serde_json::Value>,
    raw: bool,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut cmd = std::process::Command::new("jq");
    cmd.arg("-c");
    // `-r` decides from the runtime value, so it would print a present Opt<Str> raw and an
    // absent one as the word null. The rule here is the type's, as on every other backend.
    if raw {
        cmd.arg("-r");
    }
    if value.is_none() {
        cmd.arg("-n");
    }
    let mut child = cmd
        .arg(source)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run `jq`: {e}"))?;

    let text = value.map(|v| v.to_string()).unwrap_or_default();
    child.stdin.take().expect("piped").write_all(text.as_bytes())?;

    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(format!("jq failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8(out.stdout)?)
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

/// Runs through `python3`. Like `node`, this is an interpreter that has to be on the machine
/// rather than one vendored into the build.
fn run_py(
    source: &str,
    value: Option<&serde_json::Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("program.py");
    std::fs::write(&path, source)?;

    let mut child = std::process::Command::new("python3")
        .arg(&path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run `python3`: {e}"))?;

    let text = value.map(|v| v.to_string()).unwrap_or_default();
    child.stdin.take().expect("piped").write_all(text.as_bytes())?;

    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(format!("python3 failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

/// Runs through `go run`, which compiles and executes in one step. Go has no interpreter, so
/// this is the second backend that needs a real toolchain; like `node` and `cc`, a missing one
/// is an error rather than a skipped backend.
fn run_go(
    source: &str,
    value: Option<&serde_json::Value>,
) -> Result<String, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("main.go");
    std::fs::write(&path, source)?;

    let mut child = std::process::Command::new("go")
        .arg("run")
        .arg(&path)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run `go`: {e}"))?;

    let text = value.map(|v| v.to_string()).unwrap_or_default();
    child.stdin.take().expect("piped").write_all(text.as_bytes())?;

    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(format!("go failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8(out.stdout)?)
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
