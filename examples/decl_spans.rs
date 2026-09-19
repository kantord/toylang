// Temporary helper: parse a toylang program and print each top-level fn/type
// declaration's end byte offset (one per line), so a script can place `;`.
use std::io::Read;

fn main() {
    let mut src = String::new();
    std::io::stdin().read_to_string(&mut src).unwrap();
    let file = toylang::parse::parse(&src).expect("parse");
    for d in &file.defs {
        println!("{}", d.span.end);
    }
    for a in &file.aliases {
        println!("{}", a.span.end);
    }
}
