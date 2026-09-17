fn main() {
    let cases = [
        "fn f() -> Seq<Int, Stream<Int>> = 0\n\n1",
        "fn f() -> Seq<Int, Stream<Int>> = 0\n\nf()",
        "fn f(s: Seq<Int, Stream<Int>>) -> Int = 0\n\n1",
    ];
    for src in cases {
        println!("=== src: {src:?}");
        match toylang::compile(src) {
            Ok(p) => {
                println!("  COMPILE OK");
                match toylang::fmt(src) {
                    Ok(s) => println!("  FMT: {s}"),
                    Err(e) => println!("  FMT ERR: {e}"),
                }
            }
            Err(e) => println!("  COMPILE ERR: {e}"),
        }
    }
}
