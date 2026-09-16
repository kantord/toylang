mod support;

#[test]
fn scratch_dump() {
    for src in [
        "[[3, 1, 2], [5, 4]] | map(reverse(.))\n",
        "[[1, 2]] | map((.) + [3])\n",
        "some([3, 1, 2]) | map(sort(.))\n",
        "some([3, 1, 2]) | map(reverse(.))\n",
        "enum E = A(Vec<Int>) | B\nmatch A([3, 1, 2]) { A(x) => sort(x), B => [] }\n",
    ] {
        match toylang::compile(src) {
            Ok(program) => {
                let out = toylang::emit_rs::emit(&program);
                println!("=== SRC: {src:?}");
                for line in out.lines() {
                    if line.contains("tl_sort") || line.contains("tl_reverse")
                        || line.contains("tl_flatten") || line.contains("concat")
                        || line.contains("fn main") || line.contains("into_iter")
                        || line.contains("iter().map")
                    {
                        println!("    {line}");
                    }
                }
            }
            Err(e) => println!("=== SRC: {src:?} COMPILE ERROR: {e}"),
        }
    }
}
