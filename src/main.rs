use std::io::Read;
use std::process::ExitCode;

const USAGE: &str = "usage: toylang <run|emit> FILE";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [cmd, path] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::FAILURE;
    };

    let src = match std::fs::read_to_string(path) {
        Ok(src) => src,
        Err(e) => {
            eprintln!("toylang: {path}: {e}");
            return ExitCode::FAILURE;
        }
    };

    let result = match cmd.as_str() {
        "run" => run(&src),
        "emit" => toylang::compile(&src).map(|c| c.lua).map_err(Into::into),
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
fn run(src: &str) -> Result<String, Box<dyn std::error::Error>> {
    if toylang::compile(src)?.input.is_none() {
        return toylang::run(src);
    }
    let mut stdin = String::new();
    std::io::stdin().read_to_string(&mut stdin)?;
    toylang::run_with_input(src, Some(&stdin))
}
