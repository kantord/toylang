# Escalation: the serialized form of a variant under the capital-name rule

gh:156 splits a variant's spelling in two: the declared name is the matcher, capital and used
in a pattern (`Circle`), and the value is built with the lowercase constructor (`circle`).
That split is unambiguous. What it implies for the JSON wire form was not settled by the
issue, which is a grilling round re-examining exactly this convention.

## The question

A `Circle` value serializes to JSON. As what -- the matcher's capital (`"Circle"`) or the
constructor's lowercase (`"circle"`)?

## Why capital, and why it is forced

The backend readers and printers switch on the *declared* variant name. Once declarations are
capital, a reader emits/accepts `"Circle"` whether or not that was separately decided. The
draft records the same answer: the 2026-08-28 revision says "The JSON form is the name
verbatim (`"Active"`)", and the 2026-08-29 auto-matchers revision that follows does not
reverse it. So capital is both what the code does and what the design document says.

The alternative -- a lowercase wire (`"circle"`) -- would mean changing every backend's reader
and printer to lower the declared name, and it contradicts the draft. It was rejected.

## Cost and follow-up

- The wire form of existing enum data changes: `{"circle":{"r":1}}` becomes
  `{"Circle":{"r":1}}`, a unit variant `"active"` becomes `"Active"`. This is a breaking
  change to any data already persisted in the old form.
- `docs/` still describes the pre-split convention ("a variant name is data, so it is
  lowercase") and its enum fragments declare lowercase variants that no longer compile. The
  docs mega-test (`just test`) is skipped by `just check`, so this lane is green, but the
  enum pages under `docs/tutorial`, `docs/reference`, `docs/guides`, and `docs/adr/0009` need
  updating before a promotion that runs `just test`.
- The prelude's `Opt`/`Result` keep lowercase variants, exempt from the capital rule, until
  the migration already tracked as gh:165.

## Conservative continuation taken

Capital wire, per the draft and the code as delivered. The migration of the corpus and the
unit tests to the new spelling is complete; `just check`, `just fmt`, and `just clippy` are
green on the work touched.
