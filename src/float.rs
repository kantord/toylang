//! How a Float is spelled as a number literal (ADR 0007).
//!
//! Every backend emits a literal through here rather than its own target's float formatting,
//! which is what lets the corpus's agreement harness compare their output byte for byte once
//! a second backend carries Float (kantord/toylang#149). Rust's `Display` for `f64` is the
//! shortest decimal that round-trips to the same double -- exactly what a literal needs to be
//! exact on every target, and what JS's own `String(number)` produces for the same value.

/// A finite binary64 as a number literal. Non-finite values have no literal spelling (they are
/// produced by arithmetic, not written), so one arriving here is a bug upstream.
pub fn lit(n: f64) -> String {
    if !n.is_finite() {
        panic!("cannot write a non-finite value as a float literal");
    }
    n.to_string()
}
