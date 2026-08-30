//! The Int64 surface (kantord/toylang#83): what the checker refuses, and the wrapping edges
//! the corpus cannot carry.
//!
//! The happy paths live in the corpus (tests/corpus/int64_*.yaml) like every other feature's.
//! What is here instead is the 2^63 boundary: jq computes Int64 in IEEE doubles, exact only
//! within +/-2^53, so a program whose values cross that line cannot be a corpus case -- the
//! corpus requires all seven backends to agree, and past 2^53 jq honestly does not. The other
//! six do, and that agreement is pinned here with jq's divergence beside it rather than hidden.

use toylang::Backend;

/// `Int64::MAX + 1` wraps to `Int64::MIN` on every backend whose integers are exact at 64
/// bits: everything but jq.
#[test]
fn int64_wraps_at_the_2_63_boundary() {
    let src = "fn big() -> Int64 = 9223372036854775807\n\nbig() + 1\n";
    for backend in Backend::ALL {
        if backend == Backend::Jq {
            continue;
        }
        let out = toylang::run_on(src, None, backend).unwrap();
        assert_eq!(
            out,
            "-9223372036854775808\n",
            "{} does not wrap at 2^63",
            backend.name()
        );
    }
}

/// `MIN / -1` and `MIN % -1` wrap like everything else, extending ADR 0006's one-way-to-fail
/// rule to the width where the underlying hardware division would otherwise trap.
#[test]
fn int64_min_over_minus_one_wraps() {
    let div = "fn min() -> Int64 = -9223372036854775807\n\n(min() - 1) / i64(0 - 1)\n";
    let rem = "fn min() -> Int64 = -9223372036854775807\n\n(min() - 1) % i64(0 - 1)\n";
    for backend in Backend::ALL {
        if backend == Backend::Jq {
            continue;
        }
        assert_eq!(
            toylang::run_on(div, None, backend).unwrap(),
            "-9223372036854775808\n",
            "{}: MIN / -1 is MIN",
            backend.name()
        );
        assert_eq!(
            toylang::run_on(rem, None, backend).unwrap(),
            "0\n",
            "{}: MIN % -1 is 0",
            backend.name()
        );
    }
}

/// The documented boundary, observed rather than assumed: past 2^53 jq's doubles round, so
/// the same program that wraps everywhere else prints the unwrapped, rounded sum here. If
/// this snapshot ever changes, jq's Int64 story changed with it.
#[test]
fn jq_int64_is_inexact_past_2_53() {
    let src = "fn big() -> Int64 = 9223372036854775807\n\nbig() + 1\n";
    let out = toylang::run_on(src, None, Backend::Jq).unwrap();
    assert_eq!(out, "9223372036854776000\n");
}

/// No implicit widening: the two integer types never meet in one operator, and the error
/// names the bridge.
#[test]
fn arithmetic_does_not_mix_the_widths() {
    insta::assert_snapshot!(
        toylang::compile("fn big() -> Int64 = 5\n\n1 + big()")
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

#[test]
fn comparison_does_not_mix_the_widths() {
    insta::assert_snapshot!(
        toylang::compile("fn big() -> Int64 = 5\n\nbig() < 2 + 2")
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

/// A too-big literal with no expectation stays an error: nothing guesses Int64.
#[test]
fn a_wide_literal_needs_a_position_that_expects_int64() {
    insta::assert_snapshot!(
        toylang::compile("600851475143")
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

/// `i64` takes an Int, so its own argument obeys the 32-bit literal rule: a wide value enters
/// as a literal only where an Int64 is already expected, never through the bridge.
#[test]
fn i64_takes_an_int() {
    insta::assert_snapshot!(
        toylang::compile("i64(9000000000)")
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

/// Reading an Int64 back off the wire is refused until its codec is decided
/// (JS parses JSON numbers into doubles), the same reversible direction Opt takes.
#[test]
fn input_cannot_be_int64() {
    insta::assert_snapshot!(
        toylang::compile("fn f(n: Int64) -> Int64 = n\n\nf(input)")
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}

#[test]
fn inputs_cannot_carry_int64() {
    insta::assert_snapshot!(
        toylang::compile(
            "fn f(s: Stream<{ts: Int64}>) -> Stream<{ts: Int64}> = s\n\njsonlines(f(inputs))"
        )
        .map(|_| ())
        .unwrap_err()
        .to_string()
    );
}

/// `str` stays Int-only: an Int64 result prints bare as the program's own output, and
/// whether `str` should widen is its own question, not decided here.
#[test]
fn str_does_not_take_an_int64() {
    insta::assert_snapshot!(
        toylang::compile("fn big() -> Int64 = 5\n\nstr(big())")
            .map(|_| ())
            .unwrap_err()
            .to_string()
    );
}
