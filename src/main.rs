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
        "run" => toylang::run(&src),
        "emit" => toylang::compile(&src).map(|(lua, _)| lua).map_err(Into::into),
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
