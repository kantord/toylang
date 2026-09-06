use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use anyhow::{Context, Result};
use toylang::Backend;
use toylang::fmt_tree::{self, Mode};

const USAGE: &str = "usage: toylang <run|emit> FILE [lua|js|jq|go|py|llvm]\n       toylang build FILE\n       toylang fmt FILE\n       toylang fmt [--write]\n       toylang --explain-offload <run|emit|build> FILE [lua|js|jq|go|py|llvm]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    // `--explain-offload` is a leading flag on the file-naming commands: strip it, remember
    // it, and dispatch the rest as usual.
    let (args, explain) = if args.first() == Some(&"--explain-offload") {
        (&args[1..], true)
    } else {
        (args.as_slice(), false)
    };
    // The project-wide forms take no file, so they are read off first; everything else, `fmt
    // FILE` included, needs one file read before it can dispatch.
    match args {
        ["fmt"] => fmt_project(Mode::Check),
        ["fmt", "--write"] => fmt_project(Mode::Write),
        _ => on_file(args, explain),
    }
}

/// The project-wide `fmt`: bare, it reports and writes nothing; with `--write`, it rewrites what
/// it reports. Both exit nonzero when the tree was not already formatted, so either can gate a
/// commit, and the report is one path per line so a caller can pipe it somewhere.
fn fmt_project(mode: Mode) -> ExitCode {
    let root = Path::new(".");
    let report = fmt_tree::run(root, mode);
    // `./` on the front of every line is noise: the walk started here, and the reader knows.
    let show = |p: &'_ Path| p.strip_prefix(root).unwrap_or(p).display().to_string();
    for path in &report.changed {
        println!("{}", show(path));
    }
    for (path, e) in &report.failed {
        eprintln!("toylang: {}: {e}", show(path));
    }
    if report.is_clean() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

/// Every command that names one file to read: `run`, `emit`, `build`, and `fmt`'s filter form.
fn on_file(args: &[&str], explain: bool) -> ExitCode {
    let (cmd, path, backend) = match args {
        [cmd, path] => (*cmd, *path, Backend::Lua),
        [cmd, path, name] => {
            let name = if *name == "llvm" { "native" } else { name };
            let Some(backend) = Backend::from_name(name) else {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            };
            (*cmd, *path, backend)
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

    // The offload explanation is a diagnostic: it goes to stderr, so the command's own
    // output -- the run's stdout, the emitted source -- is left untouched. A compile that
    // fails is reported by the dispatch below; nothing is printed here.
    if explain && matches!(cmd, "run" | "emit" | "build") {
        if let Ok(program) = toylang::compile(&src) {
            eprint!("{}", toylang::offload::explain(&program));
        }
    }

    let result = match cmd {
        "run" => run(&src, backend),
        "emit" => match toylang::compile(&src) {
            Err(e) => Err(e.into()),
            Ok(p) => backend.emit(&p).map_err(anyhow::Error::msg),
        },
        "build" => build(&src, path).map(|out| format!("{}\n", out.display())),
        "fmt" => toylang::fmt(&src).map_err(anyhow::Error::from),
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
fn build(src: &str, path: &str) -> Result<PathBuf> {
    let stem = std::path::Path::new(path)
        .file_stem()
        .context("the source file has no name")?
        .to_owned();
    let out = PathBuf::from(stem);
    toylang::link(&toylang::compile(src)?, &out)?;
    Ok(out)
}

/// stdin is only read when the program says it reads input, so a program that does not is not
/// left waiting on a terminal.
fn run(src: &str, backend: Backend) -> Result<String> {
    // A program reading `lines` also takes the live branch: it needs the real stdin left alone,
    // not drained into a Rust String, so each backend can read it incrementally for itself.
    // `inputs` used to always need the same up-front read `input` does, since every backend
    // materialized it into a Vec before the program body ran; a backend whose generated code now
    // reads `inputs` for itself one record at a time (`streams_inputs`) gets the same live
    // treatment `lines` always had, and everything else still needs the whole thing in hand
    // before `run_on` can validate it.
    let program = toylang::compile(src)?;
    let needs_upfront_read =
        program.input.is_some() || (program.inputs.is_some() && !toylang::streams_inputs(&program));
    if !needs_upfront_read {
        return toylang::run_on(src, None, backend);
    }
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;
    toylang::run_on(src, Some(&stdin), backend)
}
