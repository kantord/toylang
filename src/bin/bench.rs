//! The benchmark harness: compiles a `benches/programs/<name>.toy` benchmark once, prepares one
//! runnable command per backend -- building Go, Rust, and Native ahead of time and retargeting
//! Lua at the system interpreter, since hyperfine can only time a real subprocess -- and drives
//! `hyperfine` over all of them. Design: plans/benchmark-plan.md and plans/benchmark-tooling-
//! spike.md.
//!
//! Scope today: a benchmark reads at most one `Int` value from stdin, or nothing. That is every
//! benchmark this harness runs so far (see plans/benchmark-plan.md's note on why the CLBG
//! string-processing tasks are not there yet); a benchmark needing a richer input type, or
//! `inputs`/`lines`, needs this harness extended first, most of all the Lua backend's `t_input`
//! injection below, which assumes an Int.

use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use toylang::Backend;

fn main() -> Result<()> {
    let name = std::env::args()
        .nth(1)
        .context("usage: bench <name> (a file under benches/programs/)")?;

    let src_path = format!("benches/programs/{name}.toy");
    let source =
        std::fs::read_to_string(&src_path).with_context(|| format!("could not read {src_path}"))?;
    let program = toylang::compile(&source).map_err(|e| anyhow::anyhow!("{src_path}: {e}"))?;

    let input_path = format!("benches/inputs/{name}.txt");
    let input = match std::fs::read_to_string(&input_path) {
        Ok(text) => Some(text.trim().parse::<i64>().with_context(|| {
            format!("{input_path}: today's harness only feeds a single Int input")
        })?),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(e).context(input_path),
    };

    let work = tempfile::tempdir()?;
    let mut names = Vec::new();
    let mut commands = Vec::new();
    for backend in Backend::ALL {
        match prepare(backend, &program, work.path(), input) {
            Ok(cmd) => {
                names.push(backend.name().to_string());
                commands.push(cmd);
            }
            Err(e) => eprintln!("bench: skipping {}: {e:#}", backend.name()),
        }
    }
    if commands.is_empty() {
        bail!("no backend could be prepared for {name}");
    }

    std::fs::create_dir_all("benches/results")?;
    let mut hyperfine = Command::new("hyperfine");
    hyperfine.arg("--shell=none").arg("--warmup").arg("3");
    if let Some(_n) = input {
        hyperfine.arg("--input").arg(&input_path);
    }
    for (name, cmd) in names.iter().zip(&commands) {
        hyperfine.arg("--command-name").arg(name).arg(cmd);
    }
    hyperfine
        .arg("--export-markdown")
        .arg(format!("benches/results/{name}.md"))
        .arg("--export-json")
        .arg(format!("benches/results/{name}.json"));

    let status = hyperfine.status().context("could not run `hyperfine`")?;
    if !status.success() {
        bail!("hyperfine failed: {status}");
    }
    Ok(())
}

/// Builds (where a build step exists) and returns the shell-none command line hyperfine should
/// time for `backend`, as one argv-shaped string. Compiling and, for Go, linking happen here,
/// outside anything hyperfine measures -- the whole reason `run_go` and a bare `emit` are not
/// reused as-is (plans/benchmark-tooling-spike.md).
fn prepare(
    backend: Backend,
    program: &toylang::tir::Program,
    work: &Path,
    input: Option<i64>,
) -> Result<String> {
    match backend {
        Backend::Lua => {
            let mut source = String::new();
            if let Some(n) = input {
                // The subprocess has no host setting the `t_input` global the way the embedded
                // interpreter does (`run_lua` in src/lib.rs) -- so it is set in the script text
                // itself before the emitted body, the same variable name emit_lua::INPUT names.
                source.push_str(&format!("t_input = {n}\n"));
            }
            source.push_str(&backend.emit(program).map_err(anyhow::Error::msg)?);
            let path = work.join("binary.lua");
            std::fs::write(&path, source)?;
            Ok(format!("lua5.4 {}", path.display()))
        }
        Backend::Js => {
            let path = work.join("binary.js");
            std::fs::write(&path, backend.emit(program).map_err(anyhow::Error::msg)?)?;
            Ok(format!("node {}", path.display()))
        }
        Backend::Py => {
            let path = work.join("binary.py");
            std::fs::write(&path, backend.emit(program).map_err(anyhow::Error::msg)?)?;
            Ok(format!("python3 {}", path.display()))
        }
        Backend::Jq => {
            let filter = backend.emit(program).map_err(anyhow::Error::msg)?;
            let mut argv = vec![
                "jq".to_string(),
                "--unbuffered".to_string(),
                "-c".to_string(),
            ];
            if input.is_none() {
                argv.push("-n".to_string());
            }
            argv.push(format!("'{filter}'"));
            Ok(argv.join(" "))
        }
        Backend::Go => {
            let dir = work.join("go");
            std::fs::create_dir_all(&dir)?;
            let src = dir.join("main.go");
            std::fs::write(&src, backend.emit(program).map_err(anyhow::Error::msg)?)?;
            let bin = dir.join("binary");
            let status = Command::new("go")
                .arg("build")
                .arg("-o")
                .arg(&bin)
                .arg(&src)
                .status()
                .context("could not run `go build`")?;
            if !status.success() {
                bail!("go build failed: {status}");
            }
            Ok(bin.display().to_string())
        }
        Backend::Native => {
            let exe = work.join("binary-native");
            toylang::link(program, &exe)?;
            Ok(exe.display().to_string())
        }
        Backend::Rust => {
            let exe = work.join("binary-rust");
            let source = backend.emit(program).map_err(anyhow::Error::msg)?;
            toylang::link_rust(&source, &exe)?;
            Ok(exe.display().to_string())
        }
    }
}
