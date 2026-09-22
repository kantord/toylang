//! The v1 mutation rule (`single_consuming_use` in src/emit_rs.rs): a Vec/Str binding read
//! exactly once, and only by a consuming builtin (Sort/Reverse/Flatten) or a `+`-concat, is
//! mutated in place at emission instead of cloned. The tests below exercise the rule at each
//! binding form it applies to (a `let`, a `map` element, a match arm's payload), and check that
//! a second read of the same binding turns the owned form back off -- undercounting a use is the
//! actual failure the rule can have, since it would move or mutate a value someone else reads.

mod support;

fn emit(src: &str) -> String {
    let program = toylang::compile(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
    toylang::emit_rs::emit(&program)
}

#[test]
fn map_element_single_consuming_reverse_is_owned() {
    let out = emit("[[3, 1, 2], [5, 4]] | map(reverse(.))\n");
    assert!(
        out.contains("into_iter().map"),
        "map should move its element:\n{out}"
    );
    assert!(
        out.contains("tl_reverse_owned("),
        "single-consuming reverse should be owned:\n{out}"
    );
}

#[test]
fn map_element_single_consuming_concat_extends_in_place() {
    let out = emit("[[1, 2]] | map((.) + [3])\n");
    assert!(
        out.contains(".extend("),
        "single-consuming concat should extend in place:\n{out}"
    );
}

#[test]
fn let_binding_single_consuming_sort_is_owned() {
    let out = emit("fn f() -> Vec<Int> =\n    let xs = [3, 1, 2]\n\n    sort(xs)\n\n\nf()\n");
    assert!(
        out.contains("let mut"),
        "a single-consuming let binding should be mut:\n{out}"
    );
    assert!(
        out.contains("tl_sort_owned("),
        "single-consuming sort should be owned:\n{out}"
    );
}

#[test]
fn match_arm_payload_single_consuming_sort_is_owned() {
    let out = emit(
        "enum E { A(Vec<Int>), B }\n\nfn f(e: E) -> Vec<Int> =\n  e | A -> sort(.) or B -> [];\n\nf(a([3, 1, 2]))\n",
    );
    assert!(
        out.contains("tl_sort_owned("),
        "a single-consuming payload should be owned:\n{out}"
    );
}

/// Same local as `let_binding_single_consuming_sort_is_owned`, but `sort(xs)` is no longer its
/// only read: `+ xs` reads it again, so the sort call must keep the borrowing form even though
/// it's still the only consuming use.
#[test]
fn let_binding_read_twice_is_not_owned() {
    let out = emit("fn f() -> Vec<Int> =\n    let xs = [3, 1, 2]\n\n    sort(xs) + xs\n\n\nf()\n");
    assert!(
        !out.contains("tl_sort_owned("),
        "a multi-use local must not be owned:\n{out}"
    );
    assert!(
        out.contains("tl_sort(&"),
        "a multi-use local's sort should still borrow:\n{out}"
    );
}

#[test]
fn map_element_read_twice_is_not_owned() {
    let out = emit("[[3, 1, 2]] | map(sort(.) + (.))\n");
    assert!(
        !out.contains("into_iter"),
        "a multi-use map element must not be moved:\n{out}"
    );
    assert!(
        !out.contains("tl_sort_owned("),
        "a multi-use map element must not be owned:\n{out}"
    );
}
