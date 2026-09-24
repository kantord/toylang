//! The v1 mutation rule on the Lua backend. Lua tables are shared by reference, so a local that
//! passes the rule's use-counting (`mutation::single_consuming_use`) is not yet safe to sort,
//! reverse or extend in place: the table it names may also be a record's field, another Vec's
//! element, the parameter a caller passed in, or a value bound to a second name. These tests pin
//! both directions -- the owned form is emitted where nothing else can observe the change, and
//! is not where something can -- while tests/corpus/mutation_*.yaml pins that the programs still
//! print the same thing on every backend.

fn emit(src: &str) -> String {
    let program = toylang::compile(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
    toylang::emit_lua::emit(&program)
}

fn assert_owned(src: &str, form: &str) {
    let out = emit(src);
    assert!(out.contains(form), "expected {form} in:\n{out}");
}

/// No `_owned` call at all: the helper definitions are only emitted alongside a use, so their
/// absence is the check.
fn assert_copies(src: &str) {
    let out = emit(src);
    assert!(!out.contains("_owned"), "expected no owned form in:\n{out}");
}

#[test]
fn let_binding_single_consuming_sort_is_owned() {
    assert_owned(
        "fn f() -> Vec<Int> =\n    let xs = [3, 1, 2]\n\n    sort(xs)\n\n\nf()\n",
        "tl_sort_owned(t_",
    );
}

#[test]
fn let_binding_single_consuming_reverse_is_owned() {
    assert_owned(
        "fn f() -> Vec<Int> =\n    let xs = [3, 1, 2]\n\n    reverse(xs)\n\n\nf()\n",
        "tl_reverse_owned(t_",
    );
}

#[test]
fn let_binding_single_consuming_concat_appends_in_place() {
    assert_owned(
        "fn f() -> Vec<Int> =\n    let xs = [3, 1, 2]\n\n    xs + [9]\n\n\nf()\n",
        "tl_append_owned(t_",
    );
}

/// Only the left operand can be extended in place; the right one keeps the copying helper.
#[test]
fn right_operand_of_concat_is_not_owned() {
    assert_copies("fn f() -> Vec<Int> =\n    let xs = [3, 1, 2]\n\n    [9] + xs\n\n\nf()\n");
}

#[test]
fn map_element_of_a_fresh_vec_is_owned() {
    assert_owned(
        "fn f() -> Vec<Vec<Int>> =\n    let rows = [[3, 1, 2], [5, 4]]\n\n    rows | map(reverse(.))\n\n\nf()\n",
        "tl_reverse_owned(t_",
    );
}

#[test]
fn match_arm_payload_of_a_fresh_enum_is_owned() {
    assert_owned(
        "enum E { A(Vec<Int>), B }\n\nfn f() -> Vec<Int> =\n  a([3, 1, 2]) | A -> sort(.) or B -> [];\n\nf()\n",
        "tl_sort_owned(t_",
    );
}

#[test]
fn a_producer_that_builds_a_new_vec_is_fresh() {
    assert_owned(
        "fn f() -> Vec<Int> =\n    let xs = collect(range(4))\n\n    reverse(xs)\n\n\nf()\n",
        "tl_reverse_owned(t_",
    );
}

/// Same local as `let_binding_single_consuming_sort_is_owned`, read again after the sort.
#[test]
fn let_binding_read_twice_is_not_owned() {
    assert_copies("fn f() -> Vec<Int> =\n    let xs = [3, 1, 2]\n\n    sort(xs) + xs\n\n\nf()\n");
}

#[test]
fn map_element_read_twice_is_not_owned() {
    assert_copies(
        "fn f() -> Vec<Vec<Int>> =\n    let rows = [[3, 1]]\n\n    rows | map(sort(.) + (.))\n\n\nf()\n",
    );
}

/// `ys` is the only reader of nothing: it names the table `xs` names, so sorting it in place
/// would reorder `xs`.
#[test]
fn a_second_name_for_the_same_table_is_not_owned() {
    assert_copies(
        "fn f() -> { a: Vec<Int>, b: Vec<Int> } =\n    let xs = [3, 1, 2]\n\n    let ys = xs\n\n    { a: sort(ys), b: xs }\n\n\nf()\n",
    );
}

#[test]
fn a_parameter_is_not_owned() {
    assert_copies("fn g(v: Vec<Int>) -> Vec<Int> =\n    let w = v\n\n    sort(w)\n\n\ng([3, 1, 2])\n");
}

#[test]
fn a_record_field_is_not_owned() {
    assert_copies(
        "fn f() -> Vec<Int> =\n    let r = { v: [3, 1, 2] }\n\n    let w = r.v\n\n    sort(w)\n\n\nf()\n",
    );
}

#[test]
fn an_element_of_a_vec_read_through_an_index_is_not_owned() {
    assert_copies(
        "fn f() -> Vec<Int> =\n    let xss = [[3, 1], [2]]\n\n    let first = xss[0]!\n\n    sort(first)\n\n\nf()\n",
    );
}

/// A call's result is not known to be fresh: the callee may hand back its own parameter.
#[test]
fn a_calls_result_is_not_owned() {
    assert_copies(
        "fn id(v: Vec<Int>) -> Vec<Int> = v;\n\n\nfn f() -> Vec<Int> =\n    let xs = id([3, 1, 2])\n\n    sort(xs)\n\n\nf()\n",
    );
}

/// Elements built once each are fresh; the same table in two slots is not.
#[test]
fn a_map_element_that_appears_twice_is_not_owned() {
    assert_copies(
        "fn f() -> Vec<Vec<Int>> =\n    let xs = [3, 1, 2]\n\n    let shared = [xs, xs]\n\n    shared | map(reverse(.))\n\n\nf()\n",
    );
}

#[test]
fn a_payload_also_held_by_another_name_is_not_owned() {
    assert_copies(
        "enum E { A(Vec<Int>), B }\n\nfn f() -> { x: Vec<Int>, y: E } =\n    let e = a([3, 1, 2])\n\n    { x: e | A -> sort(.) or B -> [], y: e }\n\n\nf()\n",
    );
}

/// One read in the source, but the `map` body runs it once per element.
#[test]
fn a_local_read_inside_a_map_body_is_not_owned() {
    assert_copies(
        "fn f() -> Vec<Vec<Int>> =\n    let xs = [3, 1, 2]\n\n    [1, 2] | map(reverse(xs))\n\n\nf()\n",
    );
    assert_copies(
        "fn f() -> Vec<Vec<Int>> =\n    let xs = [3, 1, 2]\n\n    [1, 2] | map(xs + [.])\n\n\nf()\n",
    );
}

#[test]
fn a_local_read_inside_a_closure_body_is_not_owned() {
    assert_copies(
        "fn count_where({ items, pred }: { items: Vec<Int>, pred: Int -> Bool }) -> Int =\n    items | select pred | length(.);\n\n\nfn f() -> Int =\n    let xs = [3, 1, 2]\n\n    count_where { items: [1, 2, 3], pred: $ == reverse(xs)[0]! }\n\n\nf()\n",
    );
}

/// The owned local is bound outside the `map` but is the map's own source, not read in its body.
#[test]
fn a_local_used_as_the_source_of_a_map_stays_owned_for_the_elements() {
    assert_owned(
        "fn f() -> Vec<Vec<Int>> =\n    let rows = [[3, 1, 2]]\n\n    rows | map(sort(.))\n\n\nf()\n",
        "tl_sort_owned(t_",
    );
}
