use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "usage: toylang <run|emit> FILE [lua|js]";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (cmd, path, backend) = match args.as_slice() {
        [cmd, path] => (cmd, path, toylang::Backend::Lua),
        [cmd, path, name] => match name.as_str() {
            "lua" => (cmd, path, toylang::Backend::Lua),
            "js" => (cmd, path, toylang::Backend::Js),
            _ => {
                eprintln!("{USAGE}");
                return ExitCode::FAILURE;
            }
        },
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

    let result = match cmd.as_str() {
        "run" => run(&src, backend),
        "emit" => toylang::compile(&src).map(|p| backend.emit(&p)).map_err(Into::into),
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

/// stdin is only read when the program says it reads input, so a program that does not is not
/// left waiting on a terminal.
fn run(src: &str, backend: toylang::Backend) -> Result<String, Box<dyn std::error::Error>> {
    if toylang::compile(src)?.input.is_none() {
        return toylang::run_on(src, None, backend);
    }
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;
    toylang::run_on(src, Some(&stdin), backend)
}
