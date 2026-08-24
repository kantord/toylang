//! What the checker refuses, and what unwrapping does to a type.
//!
//! The programs every backend has to refuse at *runtime* used to live here, because the corpus
//! could only describe programs that succeed. It says `refuses: true` now, so they moved: see
//! tests/corpus/unwrap_absent.yaml, div_by_zero.yaml and rem_by_zero.yaml.

use toylang::Backend;

/// The type is what decides whether output is raw, so unwrapping changes it: `Opt<Str>` prints
/// as JSON and the `Str` behind it prints raw.
#[test]
fn unwrapping_changes_how_a_string_prints() {
    let wrapped = toylang::run_on(r#"["ada", "bo"][0]"#, None, Backend::Lua).unwrap();
    let unwrapped = toylang::run_on(r#"["ada", "bo"][0]!"#, None, Backend::Lua).unwrap();
    assert_eq!(wrapped, "\"ada\"\n");
    assert_eq!(unwrapped, "ada\n");
}

/// `str` is a builtin, so a program cannot define its own and silently mean something else.
#[test]
fn a_builtin_cannot_be_redefined() {
    let err = toylang::compile("fn str(x: Int) -> Str = x\nstr(1)")
        .map(|_| ())
        .unwrap_err()
        .to_string();
    insta::assert_snapshot!(err);
}

#[test]
fn str_takes_an_int() {
    insta::assert_snapshot!(
        toylang::compile(r#"str("a")"#).map(|_| ()).unwrap_err().to_string()
    );
}

/// `+` is the one operator whose meaning depends on its operands, and nothing is coerced.
#[test]
fn plus_does_not_mix_its_operands() {
    insta::assert_snapshot!(
        toylang::compile(r#"1 + "a""#).map(|_| ()).unwrap_err().to_string()
    );
}

/// The condition is exactly one Bool. This is where jq runs both branches and gets two answers;
/// here it does not typecheck.
#[test]
fn a_condition_must_be_a_bool() {
    insta::assert_snapshot!(
        toylang::compile(r#""a" if 1 else "b""#).map(|_| ()).unwrap_err().to_string()
    );
}

/// Both branches have to agree, since the conditional is an expression with one type.
#[test]
fn both_branches_must_agree() {
    insta::assert_snapshot!(
        toylang::compile(r#""a" if 1 == 1 else 2"#).map(|_| ()).unwrap_err().to_string()
    );
}

/// Go folds constant arithmetic exactly and will not compile a result that does not fit, so it
/// was the first backend that could not go along with a literal wider than the type. The other
/// four agreed on the wrong answer, each holding the literal in its own wider representation
/// until an operator wrapped it, which is agreement by coincidence rather than by rule.
#[test]
fn an_int_literal_has_to_fit_in_an_int() {
    insta::assert_snapshot!(
        toylang::compile("str(9999999999)").map(|_| ()).unwrap_err().to_string()
    );
}

/// A minus directly on a literal is part of the literal, so the most negative Int is writable
/// even though its magnitude is one past the most positive. One further and it is not.
#[test]
fn the_most_negative_int_is_writable_but_not_one_past_it() {
    assert!(toylang::compile("str(-2147483648)").is_ok());
    insta::assert_snapshot!(
        toylang::compile("str(-2147483649)").map(|_| ()).unwrap_err().to_string()
    );
}

/// The 32-bit rule has to hold at both places an Int enters, and input is the other one.
/// Before this, five backends carried the value and Go refused to decode it, which the corpus
/// would have reported as one backend broken rather than as a rule the language was not keeping.
#[test]
fn an_int_from_input_has_to_fit_too() {
    let src = "fn ts(db: {t: Int}) -> Int = db.t\n\nts(input)";
    let err = toylang::run_on(src, Some(r#"{"t": 9999999999}"#), Backend::Lua)
        .map(|_| ())
        .unwrap_err()
        .to_string();
    insta::assert_snapshot!(err);

    for backend in Backend::ALL {
        let ok = toylang::run_on(src, Some(r#"{"t": 2147483647}"#), backend);
        assert_eq!(ok.expect("the boundary value is in range"), "2147483647\n", "{}", backend.name());
    }
}
