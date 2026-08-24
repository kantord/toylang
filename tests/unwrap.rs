//! Unwrapping an absent value stops the program.
//!
//! This cannot live in the corpus, which compares the output of programs that succeed. What has
//! to be identical here is that every backend refuses, not what it prints while refusing.

use toylang::Backend;

#[test]
fn unwrapping_an_absent_value_stops_every_backend() {
    let mut ran = 0;
    for backend in Backend::ALL {
        let result = toylang::run_on("[1, 2, 3][9]!", None, backend);
        assert!(
            result.is_err(),
            "{}: unwrapping an absent value produced {:?}",
            backend.name(),
            result
        );
        ran += 1;
    }
    assert_eq!(ran, Backend::ALL.len(), "every backend has to have been tried");
}

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

#[test]
fn dividing_by_zero_stops_every_backend() {
    let mut ran = 0;
    for backend in Backend::ALL {
        for src in ["str(1 / 0)", "str(1 % 0)"] {
            assert!(
                toylang::run_on(src, None, backend).is_err(),
                "{}: {src} did not stop",
                backend.name()
            );
            ran += 1;
        }
    }
    assert_eq!(ran, Backend::ALL.len() * 2);
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
