Board row `bigint-tracking` (gh:112): Arbitrary-precision integers -- tracked, deliberately unscheduled.

This is a RESEARCH task, no compiler code changes expected. Survey how a small number of
comparable languages (pick from: Python, Rust with a crate like `num-bigint`, Go's
`math/big`, JavaScript `BigInt`) represent and expose arbitrary-precision integers: what
the surface syntax/type looks like, how it interacts with the language's normal fixed-width
int type (implicit promotion vs explicit conversion), and the performance/ergonomics
tradeoffs each made. Then sketch 2-3 concrete options for how toylang could add a BigInt
type, with real toylang code examples for each option (declaration, arithmetic, conversion
to/from the existing int type). Note open questions a future grilling round would need to
settle (e.g. does `+` overload silently or require an explicit combinator, is promotion
automatic on overflow).

Write findings to `plans/bigint-tracking-research.md` and commit it.
