// Temporary helper: read a toylang program path from argv[1], print its canonical form.
fn main() {
    let path = std::env::args().nth(1).expect("usage: _fmt_helper <file>");
    let src = std::fs::read_to_string(&path).unwrap();
    match toylang::fmt(&src) {
        Ok(formatted) => print!("{formatted}"),
        Err(e) => {
            eprintln!("ERR {path}: {e}");
            std::process::exit(2);
        }
    }
}
