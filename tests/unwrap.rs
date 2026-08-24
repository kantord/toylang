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
