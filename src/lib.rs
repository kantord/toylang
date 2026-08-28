pub mod ast;
pub mod check;
pub mod emit_go;
pub mod emit_jq;
pub mod emit_js;
pub mod emit_llvm;
pub mod emit_lua;
pub mod emit_py;
pub mod emit_rs;
pub mod error;
pub mod input;
pub mod parse;
pub mod prelude;
pub mod tags;
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
    Rust,
}

impl Backend {
    /// The backends that must agree on every corpus program. Native and Rust each joined once
    /// they could compile the whole language; until then they were kept out rather than allowed
    /// to turn the harness permanently red, tracked by tests/backend_llvm.rs and
    /// tests/backend_rust.rs respectively.
    pub const ALL: [Backend; 7] = [
        Backend::Lua,
        Backend::Js,
        Backend::Native,
        Backend::Jq,
        Backend::Go,
        Backend::Py,
        Backend::Rust,
    ];

    /// The spelling used on the command line and in a corpus case's `snapshot` list.
    pub fn from_name(name: &str) -> Option<Backend> {
        Backend::ALL.into_iter().find(|b| b.name() == name)
    }

    pub fn name(self) -> &'static str {
        match self {
            Backend::Lua => "lua",
            Backend::Js => "js",
            Backend::Native => "native",
            Backend::Jq => "jq",
            Backend::Go => "go",
            Backend::Py => "py",
            Backend::Rust => "rust",
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
            Backend::Rust => emit_rs::emit(program),
        })
    }
}

pub fn compile(src: &str) -> Result<Program, Error> {
    let mut file = parse::parse(src)?;
    prelude::inject(&mut file);
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

/// Whether the generated code for `program` reads `inputs` for itself, one record at a time,
/// rather than needing the whole thing parsed and handed over up front. A type question now,
/// not a shape guess: exactly the programs `tir::fusion` reads from `inputs` -- every backend
/// has a fused loop to emit. An aggregate like `extent(collect(inputs))` still has no way to
/// stream no matter which backend runs it. Public so `main.rs` can decide, before it has read
/// anything, whether to drain real stdin itself or hand it to `run_on` untouched.
pub fn streams_inputs(program: &tir::Program) -> bool {
    matches!(tir::fusion(program), Some(f) if f.source == tir::Source::Inputs)
}

/// Compile and run, capturing what the program printed -- except when `stdin` is `None` and
/// `streams_inputs` says the backend reads `inputs` for itself, in which case the real stdin and
/// stdout are connected straight through (`Feed::Live`) and there is nothing left to capture.
/// Capturing otherwise keeps the tests to one call: it also means output is held in memory, which
/// is fine for every program whose result has statically known extent, and is exactly the case
/// `Feed::Live` exists for on the ones that no longer do.
pub fn run_on(
    src: &str,
    stdin: Option<&str>,
    backend: Backend,
) -> Result<String, Box<dyn std::error::Error>> {
    let program = compile(src)?;

    // `stdin.is_none()` is the same convention `uses_lines` already uses below: nothing was
    // supplied, which only happens on the real command line, never from a test fixture. Only
    // then is there real live stdin worth handing straight to a subprocess backend rather than
    // something already sitting in memory to validate up front.
    let live_inputs = stdin.is_none() && streams_inputs(&program);
    // Lua is the one backend with no subprocess of its own: `run_lua` itself decides how
    // `inputs` reaches the running chunk, by injecting either a pre-populated global or a
    // per-call function, and the emitted source commits to one of those two shapes purely from
    // `tir::fusion`, independent of whether a fixture or the real command line supplied the
    // bytes. So unlike `live_inputs` above, this also has to hold for a fixture-fed test, or
    // the host would inject the global while the fused source calls the function.
    let lua_fused = matches!(backend, Backend::Lua) && streams_inputs(&program);

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

    // `inputs` is eager like `input`, not incremental like `lines`, so it normally goes through
    // the same validate-then-re-serialize step: every backend receives canonical bytes, one
    // compact JSON value per line, rather than whatever formatting the original text happened to
    // use. `live_inputs` and `lua_fused` are the exceptions -- the backend's own generated code
    // (or, for Lua, the host function it calls into) reads and validates each record for itself
    // there, which is what `tir::fusion` guarantees before `streams_inputs` says yes,
    // so there is nothing left for the host to precompute.
    let inputs_values = if live_inputs || lua_fused {
        None
    } else {
        match (&program.inputs, stdin) {
            (Some(elem_ty), Some(text)) => {
                let mut values = Vec::new();
                for line in text.lines().filter(|l| !l.trim().is_empty()) {
                    let value: serde_json::Value = serde_json::from_str(line)?;
                    input::validate(&value, elem_ty, "inputs")?;
                    values.push(value);
                }
                Some(values)
            }
            (Some(elem_ty), None) => {
                return Err(format!("this program reads inputs, of type Vec<{elem_ty}>").into());
            }
            (None, _) => None,
        }
    };

    // What a backend's stdin should be connected to. `input` always has known bytes: the
    // re-serialized, already-validated value. `lines` has known bytes only when a caller (a
    // corpus fixture, most often) supplied them directly -- and when it does, they are piped in
    // verbatim rather than pre-split, so a backend's own splitting genuinely runs against them
    // rather than being bypassed by the test that is supposed to exercise it. Only when nothing
    // was supplied, which is the real command-line case, does `lines` connect the real stdin
    // straight through, with no Rust-side buffering in between -- the one thing that would make
    // its streaming indistinguishable from having read everything first. `inputs` gets the same
    // treatment exactly when `live_inputs` says the backend can read it for itself; a fused Lua
    // program run against a fixture is not live, but still needs the raw bytes verbatim rather
    // than the eager re-serialized form, since `run_lua`'s injected function does its own
    // splitting the same way `lines` always has.
    let feed = if program.uses_lines {
        match stdin {
            Some(text) => Feed::Text(text.to_string()),
            None => Feed::Live,
        }
    } else if live_inputs {
        Feed::Live
    } else if lua_fused {
        Feed::Text(stdin.map(str::to_string).unwrap_or_default())
    } else if let Some(values) = &inputs_values {
        Feed::Text(values.iter().map(|v| v.to_string()).collect::<Vec<_>>().join("\n"))
    } else {
        Feed::Text(value.as_ref().map(|v| v.to_string()).unwrap_or_default())
    };

    match backend {
        Backend::Lua => run_lua(
            &emit_lua::emit(&program),
            value.as_ref(),
            inputs_values.as_ref(),
            program.inputs.as_ref(),
            &feed,
        ),
        Backend::Js => run_node(&emit_js::emit(&program), &feed),
        Backend::Jq => run_jq(
            &emit_jq::emit(&program),
            value.is_some(),
            program.body.ty == ty::Type::Str,
            program.uses_lines,
            &feed,
        ),
        Backend::Go => run_go(&emit_go::emit(&program), &feed),
        Backend::Py => run_py(&emit_py::emit(&program), &feed),
        Backend::Native => {
            let dir = tempfile::tempdir()?;
            let exe = dir.path().join("program");
            link(&program, &exe)?;
            run_binary(&exe, &feed)
        }
        Backend::Rust => {
            let dir = tempfile::tempdir()?;
            let exe = dir.path().join("program");
            link_rust(&emit_rs::emit(&program), &exe)?;
            run_binary(&exe, &feed)
        }
    }
}

/// What a subprocess backend's stdin is connected to.
enum Feed {
    /// Known bytes, written through a pipe: `input`'s re-serialized value, a `lines` program's
    /// fixture text verbatim, or nothing for a program that reads no stdin at all.
    Text(String),
    /// The real process stdin, inherited rather than piped. Reachable only for a `lines`
    /// program run with no fixture supplied.
    Live,
}

impl Feed {
    fn stdio(&self) -> std::process::Stdio {
        match self {
            Feed::Text(_) => std::process::Stdio::piped(),
            Feed::Live => std::process::Stdio::inherit(),
        }
    }

    fn write_to(&self, child: &mut std::process::Child) -> std::io::Result<()> {
        if let Feed::Text(text) = self {
            child.stdin.take().expect("piped").write_all(text.as_bytes())?;
        }
        Ok(())
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

/// Compile Rust source to an executable at `out`. One `rustc` call on one self-contained file,
/// no external crate and no Cargo project, the same reason `link` above is one `cc` call: the
/// generated source depends on nothing this compiler did not already assume was installed.
/// Public for the same reason as `link`: `tests/backend_rust.rs` calls it directly, separately
/// from running the result, so a genuine codegen gap (`rustc` rejects the source) is told apart
/// from a legitimate runtime refusal (`rustc` succeeds, the binary exits non-zero).
pub fn link_rust(source: &str, out: &std::path::Path) -> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let src = dir.path().join("program.rs");
    std::fs::write(&src, source)?;

    let result = std::process::Command::new("rustc")
        .arg("--edition")
        .arg("2021")
        .arg(&src)
        .arg("-o")
        .arg(out)
        .output()
        .map_err(|e| format!("could not run `rustc`: {e}"))?;
    if !result.status.success() {
        return Err(format!(
            "rustc failed to compile: {}",
            String::from_utf8_lossy(&result.stderr)
        )
        .into());
    }
    Ok(())
}

/// Runs `cmd` (already given whatever args the caller needs) connected to `feed`, and returns
/// what it printed.
///
/// When `feed` is `Feed::Live` there is no test harness reading the result back: stdout and
/// stderr are inherited straight through to the real terminal or pipe instead of being captured,
/// which is what lets a program that streams actually show output as it produces it rather than
/// all at once when it exits. `Ok(String::new())` reflects that nothing is left to hand back --
/// it already went to the real stdout directly.
fn run_subprocess(
    mut cmd: std::process::Command,
    label: &str,
    feed: &Feed,
) -> Result<String, Box<dyn std::error::Error>> {
    let live = matches!(feed, Feed::Live);
    let mut child = cmd
        .stdin(feed.stdio())
        .stdout(if live { std::process::Stdio::inherit() } else { std::process::Stdio::piped() })
        .stderr(if live { std::process::Stdio::inherit() } else { std::process::Stdio::piped() })
        .spawn()
        .map_err(|e| format!("could not run `{label}`: {e}"))?;

    feed.write_to(&mut child)?;

    if live {
        let status = child.wait()?;
        return if status.success() {
            Ok(String::new())
        } else {
            Err(format!("{label} failed").into())
        };
    }

    let out = child.wait_with_output()?;
    if !out.status.success() {
        return Err(format!("{label} failed: {}", String::from_utf8_lossy(&out.stderr)).into());
    }
    Ok(String::from_utf8(out.stdout)?)
}

fn run_binary(
    exe: &std::path::Path,
    feed: &Feed,
) -> Result<String, Box<dyn std::error::Error>> {
    run_subprocess(std::process::Command::new(exe), "the compiled program", feed)
}

/// jq puts stdin in `.`, so a program that reads no input runs with `-n` rather than waiting on
/// a terminal. `has_value` rather than the `Feed` itself, since a `lines` program run live
/// (`Feed::Live`) still has real stdin coming and must not get `-n` on that account.
fn run_jq(
    source: &str,
    has_value: bool,
    raw: bool,
    uses_lines: bool,
    feed: &Feed,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut cmd = std::process::Command::new("jq");
    // jq's stdout is fully buffered rather than line-buffered whenever it is not a terminal, the
    // same as any other libc stdio program, so a filter over `inputs` piped into another process
    // would otherwise sit on every result until the run ends. `run_subprocess` only inherits
    // stdout instead of capturing it for a live run, so this is still what makes that case show a
    // result as soon as jq produces one rather than at exit.
    cmd.arg("--unbuffered");
    cmd.arg("-c");
    // `-r` decides from the runtime value, so it would print a present Opt<Str> raw and an
    // absent one as the word null. The rule here is the type's, as on every other backend.
    if raw {
        cmd.arg("-r");
    }
    if !has_value && !uses_lines {
        cmd.arg("-n");
    }
    // Raw-input mode, needed for `[ inputs ]` to read lines rather than JSON, and forced by the
    // checker to be the only way `lines` is used in this invocation: mixing it with `input` was
    // rejected there, because `-R` changes what the whole invocation means, not just one call.
    // `-n` here is not "no input coming"; it is what raw-input mode needs regardless, since
    // `[ inputs ]` reads for itself rather than through the implicit `.` a normal run would use.
    if uses_lines {
        cmd.arg("-R").arg("-n");
    }
    cmd.arg(source);
    run_subprocess(cmd, "jq", feed)
}

fn run_lua(
    source: &str,
    value: Option<&serde_json::Value>,
    inputs_values: Option<&Vec<serde_json::Value>>,
    inputs_elem_ty: Option<&ty::Type>,
    feed: &Feed,
) -> Result<String, Box<dyn std::error::Error>> {
    let lua = mlua::Lua::new();
    if let Some(value) = value {
        lua.globals().set(emit_lua::INPUT, input::to_lua(&lua, value)?)?;
    }
    if let Some(values) = inputs_values {
        let array = serde_json::Value::Array(values.clone());
        lua.globals().set(emit_lua::INPUTS, input::to_lua(&lua, &array)?)?;
    } else if let Some(elem_ty) = inputs_elem_ty {
        // `inputs_values` is `None` here for one of two reasons: the program does not read
        // `inputs` at all (then `inputs_elem_ty` is also `None`, and neither branch runs), or
        // `run_on` recognized this as a fused program and skipped precomputing the whole Vec on
        // purpose (`live_inputs` or `lua_fused` there). mlua is embedded in this same process, so
        // there is no separate JSON parser to write in Lua source the way Rust's own is -- the
        // generated source just calls this function for the next already-validated, already-
        // converted record. Reading from `feed` rather than always the real stdin is what keeps
        // this correct for a fixture-tested fused program too: `feed` is `Feed::Text` there, the
        // same raw bytes a subprocess backend would have been piped.
        let elem_ty = elem_ty.clone();
        let reader: RefCell<Box<dyn std::io::BufRead>> = RefCell::new(match feed {
            Feed::Live => Box::new(std::io::BufReader::new(std::io::stdin())),
            Feed::Text(text) => Box::new(std::io::Cursor::new(text.clone().into_bytes())),
        });
        let next_input = lua.create_function(move |lua, ()| {
            use std::io::BufRead;
            loop {
                let mut line = String::new();
                let n = reader
                    .borrow_mut()
                    .read_line(&mut line)
                    .map_err(mlua::Error::external)?;
                if n == 0 {
                    return Ok(mlua::Value::Nil);
                }
                let line = line.strip_suffix('\n').unwrap_or(&line);
                if line.trim().is_empty() {
                    continue;
                }
                let value: serde_json::Value =
                    serde_json::from_str(line).map_err(mlua::Error::external)?;
                input::validate(&value, &elem_ty, "inputs").map_err(mlua::Error::RuntimeError)?;
                return input::to_lua(lua, &value);
            }
        })?;
        lua.globals().set(emit_lua::NEXT_INPUT, next_input)?;
    }
    // mlua is embedded in this same process, so `io.lines()` in the emitted source reads the
    // real stdin directly -- verified against a live pipe, and exactly what a `lines` program
    // run for real should do. A fixture is different: cargo test runs many tests concurrently
    // in one process, so redirecting the process's own fd 0 would race with all of them, and is
    // not done. Instead the fixture is written to a file of its own and `io.lines` is pointed at
    // it, which keeps Lua's own real line-splitting in the loop rather than reimplementing it in
    // Rust and only testing that reimplementation.
    let _fixture_dir;
    if let Feed::Text(text) = feed {
        let dir = tempfile::tempdir()?;
        let path = dir.path().join("stdin.txt");
        std::fs::write(&path, text)?;
        let path = path.to_str().expect("a temp path is valid UTF-8").to_string();
        let io: mlua::Table = lua.globals().get("io")?;
        let real_lines: mlua::Function = io.get("lines")?;
        let fixture = lua.create_function(move |_, ()| real_lines.call::<mlua::Value>(path.clone()))?;
        io.set("lines", fixture)?;
        _fixture_dir = Some(dir);
    }

    // A live run has no test harness reading the result back, the same distinction
    // `run_subprocess` makes for the OS-process backends: `print` writes straight to the real
    // stdout and flushes per call instead of buffering into a string only handed back once the
    // whole chunk finishes.
    let live = matches!(feed, Feed::Live);
    let captured = Rc::new(RefCell::new(String::new()));
    let sink = Rc::clone(&captured);
    let print = lua.create_function(move |_, s: String| {
        if live {
            let mut stdout = std::io::stdout();
            writeln!(stdout, "{s}").map_err(mlua::Error::external)?;
            stdout.flush().map_err(mlua::Error::external)?;
        } else {
            sink.borrow_mut().push_str(&s);
            sink.borrow_mut().push('\n');
        }
        Ok(())
    })?;
    lua.globals().set("print", print)?;

    lua.load(source).exec()?;

    if live {
        return Ok(String::new());
    }
    let out = captured.borrow().clone();
    Ok(out)
}

/// Runs through `python3`. Like `node`, this is an interpreter that has to be on the machine
/// rather than one vendored into the build.
fn run_py(
    source: &str,
    feed: &Feed,
) -> Result<String, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("program.py");
    std::fs::write(&path, source)?;

    let mut cmd = std::process::Command::new("python3");
    cmd.arg(&path);
    run_subprocess(cmd, "python3", feed)
}

/// Runs through `go run`, which compiles and executes in one step. Go has no interpreter, so
/// this is the second backend that needs a real toolchain; like `node` and `cc`, a missing one
/// is an error rather than a skipped backend.
fn run_go(
    source: &str,
    feed: &Feed,
) -> Result<String, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("main.go");
    std::fs::write(&path, source)?;

    let mut cmd = std::process::Command::new("go");
    cmd.arg("run").arg(&path);
    run_subprocess(cmd, "go", feed)
}

/// Runs through `node`, which must be present. A missing toolchain is an error rather than a
/// quietly skipped backend: a report that says two backends agreed when only one ran is worse
/// than no report.
fn run_node(
    source: &str,
    feed: &Feed,
) -> Result<String, Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("program.js");
    std::fs::write(&path, source)?;

    let mut cmd = std::process::Command::new("node");
    cmd.arg(&path);
    run_subprocess(cmd, "node", feed)
}

impl std::error::Error for Error {}
