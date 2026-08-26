use std::io::Read;
use std::path::PathBuf;
use std::process::ExitCode;

use toylang::Backend;

const USAGE: &str = "usage: toylang <run|emit> FILE [lua|js|jq|go|py|llvm]\n       toylang build FILE";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, path, backend) = match args.as_slice() {
        [cmd, path] => (cmd.as_str(), path, Backend::Lua),
        [cmd, path, name] => {
            let name = if name == "llvm" { "native" } else { name };
            let Some(backend) = Backend::from_name(name) else {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            };
            (cmd.as_str(), path, backend)
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("toylang: {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let result = match cmd {
        "run" => run(&src, backend),
        "emit" => match toylang::compile(&src) {
            Err(e) => Err(e.into()),
            Ok(p) => backend.emit(&p).map_err(|e| -> Box<dyn std::error::Error> { e.into() }),
        },
        "build" => build(&src, path).map(|out| format!("{}\n", out.display())),
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };

    match result {
        Ok(out) => {
            print!("{out}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("toylang: {path}: {e}");
            ExitCode::FAILURE
        }
    }
}

/// Writes the binary next to where it was invoked, named after the source file.
fn build(src: &str, path: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let stem = std::path::Path::new(path)
        .file_stem()
        .ok_or("the source file has no name")?
        .to_owned();
    let out = PathBuf::from(stem);
    toylang::link(&toylang::compile(src)?, &out)?;
    Ok(out)
}

/// stdin is only read when the program says it reads input, so a program that does not is not
/// left waiting on a terminal.
fn run(src: &str, backend: Backend) -> Result<String, Box<dyn std::error::Error>> {
    // A program reading `lines` also takes this branch: it needs the real stdin left alone, not
    // drained into a Rust String, so each backend can read it incrementally for itself. `inputs`
    // is eager like `input`, not incremental like `lines`, so it needs the same up-front read.
    let program = toylang::compile(src)?;
    if program.input.is_none() && program.inputs.is_none() {
        return toylang::run_on(src, None, backend);
    }
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;
    toylang::run_on(src, Some(&stdin), backend)
}
