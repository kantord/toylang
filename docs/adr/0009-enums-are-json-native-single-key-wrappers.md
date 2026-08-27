---
status: accepted
---

# Enum values are JSON: single-key wrappers, bare-string unit variants

The language gets Rust-inspired enums -- declared, closed, nominal, exhaustively matched --
and this ADR records the representation half, which is the hard-to-reverse part. An enum
value is not an abstract sum rendered through a codec; it *is* a canonical JSON shape, like
every other value in the language. A payload variant is the single-key wrapper,
`{"circle": {"r": 1}}`, with the payload any single type (a record when several values
travel). A unit variant is a bare string, `"active"`, so an all-unit enum is a string enum
and `{"status": "active"}`-shaped data is directly typeable. One enum type therefore spans
two JSON shapes (string or single-key object), which is unambiguous because variant names are
unique within their enum.

## Considered options

- **Abstract sum plus codec** (Rust-faithful): rejected. It would create the language's first
  unprintable ordinary value in a data language, blind the output-equality corpus to enum
  programs, and force seven backends to invent a native sum representation. Canonical JSON
  makes the backends nearly free: a wrapper is a record and a unit variant is a string, both
  of which every backend already carries.
- **Internally tagged**, `{"kind": "circle", "r": 1}`: rejected knowingly, not overlooked. It
  is what much real-world data looks like (GitHub events, Stripe, k8s), and that data is now
  not directly typeable as an enum -- reaching it needs a codec layer this decision defers.
  The wrapper won on payload generality (scalar payloads work; a tag field forces record
  payloads and reserves a field name) and on the bare-string unit form, which covers the
  wild's *most* common enum shape, the string enum.
- **Untagged / shape-matched**: cannot represent enums whose variants are structurally
  identical -- `enum Color { red, green, blue }` -- so it cannot be the general rule. It
  survives as exactly one special case: `Opt`'s value-or-`null` form predates this decision,
  which is also why `Opt` is provably not self-hostable as an enum and stays built-in.

## Consequences

- draft.md's Q29 (discriminant convention for a derived enum codec) is superseded: there is
  no codec choosing a representation, because the representation is the value.
- The sibling decisions -- variant constructors, bare-until-ambiguous naming with
  `Shape.circle` qualification, exhaustive matching through the shared arm syntax,
  monomorphic first cut -- are recorded in draft.md's
  "DECIDED: enums, nominal and JSON-native" section, which is the primary record.
- Tag-field data joins custom representations in the deferred codec layer; deciding enums
  this way is what gives that layer its first two concrete customers.
