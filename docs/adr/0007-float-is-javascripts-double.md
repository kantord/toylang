---
status: accepted
---

# Float is JavaScript's number: an IEEE 754 binary64 double

Decided 2026-08-27, ahead of implementation: no `Float` exists in the checker yet, and `3.14`
appears only in the overview page's [values list](../overview/vision.md#values).

The float type is exactly the standard double every JavaScript engine carries -- IEEE 754
binary64 -- with no alternative width and no decimal type. The supporting facts are already in
the repository: it is the one numeric representation every backend has natively (for
JavaScript and jq it is the *only* one), and the Int decision's carrying measurements
established the double's 53-bit integer ceiling as the portable envelope, with JavaScript
setting it. Picking anything else would mean emulating a second float on the two targets that
have only this one, to gain a width nothing asked for.

Not decided here, named so they are not assumed: how float literals print (six backends must
agree byte for byte, and default float formatting differs across them), what `NaN` and
`Infinity` mean in a language whose values are JSON-shaped (JSON has no spelling for either),
and reduction semantics -- draft.md's vectorization sections already lean on `fold` declaring
associativity to make reassociation legitimate, and on keeping floating-point contraction off,
but those are operation questions, not representation ones. These are tracked as
[draft.md's Q37](../../plans/questions.md#q37-how-do-floats-print-and-what-are-nan-and-infinity-in-a-json-shaped-value-model).

## Amendment: printing and the non-finite values are decided (kantord/toylang#145, #149)

The "not decided here" list above has since been decided and built. The ruling
(kantord/toylang#145, 2026-08-30) admits `NaN` and `Infinity` as values a `Float` can hold,
printed by name, and makes `Float` division by zero return `Infinity` per IEEE rather than
failing the way `Int` division does. Printing is ECMA-262 `Number::toString`: shortest
round-trip digits, fixed notation between `1e-7` and `1e21`, exponential outside it, no
trailing `.0` on integral values. Every one of the seven backends (the count above says six;
Rust-source joined later) renders the same double to the same bytes, verified against Node
over a fuzz run of about five thousand values; the native runtime carries its own formatter
in `runtime-rs` (on the `ryu-js` crate, after a C one that retried `snprintf`) because libc has
no shortest-round-trip one. See
[Float](../reference/types/float.md).
