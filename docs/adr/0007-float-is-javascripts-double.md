---
status: accepted
---

# Float is JavaScript's number: an IEEE 754 binary64 double

Decided 2026-08-27, ahead of implementation: no `Float` exists in the checker yet, and `3.14`
appears only in draft.md's values list.

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
[draft.md's Q37](../../draft.md#q37-how-do-floats-print-and-what-are-nan-and-infinity-in-a-json-shaped-value-model).
