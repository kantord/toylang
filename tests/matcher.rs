//! The matcher first cut (draft.md, the match-arms decision): `or` composes arms, guard
//! arms may be honestly partial. The corpus carries the positive behaviour; these pin the
//! refusals.

#[track_caller]
fn err(src: &str) -> String {
    toylang::compile(src).map(|_| ()).unwrap_err().to_string()
}

/// A bare expression is the chain's default, and only the last element may be one.
#[test]
fn a_bare_expression_mid_chain() {
    insta::assert_snapshot!(err(
        "1 | . == 1 -> \"one\" or \"other\" or . == 2 -> \"two\""
    ));
}

/// The two-nulls program from the decision: our Opt is untagged, so a partial chain over an
/// Opt-bodied arm would print one `null` for two different absences (arm declined, versus
/// arm matched and found nothing). Refused, with add-a-default as the way out. (The
/// decision's own data has an empty `readings`, which a literal cannot spell -- `[]` has no
/// element type -- but the refusal is about the body's type, not the data.)
#[test]
fn a_partial_chain_with_an_opt_typed_body() {
    insta::assert_snapshot!(err(
        "[{valid: 1 == 2, readings: [5]}, {valid: 1 == 1, readings: [1]}] | map(.valid -> .readings[0])"
    ));
}

/// `//` retired when `or` became the arm composer. The token no longer exists, so the old
/// spelling parses as division followed by nothing an expression can start with.
#[test]
fn the_retired_slash_slash_chain() {
    insta::assert_snapshot!(err(
        "enum Shape { point, circle{r: Int} }\n\nfn area_ish(s: Shape) -> Int = s | circle{r} -> r * r // point -> 0\n\narea_ish(Shape.point)"
    ));
}

/// A guard is a runtime Bool the checker cannot see through, so it does not count toward
/// variant coverage: this chain still has to name its missing variant.
#[test]
fn a_guard_does_not_cover_a_variant() {
    insta::assert_snapshot!(err("enum S { a, b }\n\nS.a | a -> 1 or 1 == 1 -> 2"));
}

/// Once every variant is covered nothing is left to see, the same dead-arm rule the arms
/// after `any()` already have -- and what lets a backend take a total chain's last arm
/// without a test.
#[test]
fn an_arm_after_full_coverage_can_never_match() {
    insta::assert_snapshot!(err("enum S { a }\n\nS.a | a -> 1 or 1 == 1 -> 2"));
}

/// A guard arm's left side must be a Bool.
#[test]
fn a_guard_that_is_not_a_bool() {
    insta::assert_snapshot!(err("1 | . + 1 -> 2"));
}
