# research-log

What building toylang taught us, as opposed to what we decided. `draft.md` holds the design and
`plans/` holds the build order; this holds findings, and either of those may cite a note here
rather than restating it.

One idea per file. Every note is linked from this index and from at least one sibling, because a
note nothing points at is a note nothing will find. Frontmatter follows the OKF schema: `type`,
`calendar` and a 100-250 character `description`, which is also the note's line below.

## Notes

- [A test that cannot fail is worse than no test](a-test-that-cannot-fail-is-worse-than-no-test.md)
  -- Two assertions in prototype 1 could never have gone red, because the property each claimed
  was invisible in what it observed.
- [A second type is what makes a checker falsifiable](a-second-type-is-what-makes-a-checker-falsifiable.md)
  -- With one type in the language every check passes by construction, so a type checker cannot
  be tested until a second type exists to violate it.
- [The backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md)
  -- Two prototype 1 bugs came from the emitted Lua rather than from toylang, and neither was
  visible anywhere in the front end.
- [Losing jaq's corpus means building the agreement harness](losing-jaqs-corpus-means-building-the-agreement-harness.md)
  -- Dropping the jaq fork also dropped a ready-made conformance suite, and cross-backend
  agreement testing is now toylang's problem.
- [A pure value layer dissolves jq's iteration operators](a-pure-value-layer-dissolves-jqs-iteration-operators.md)
  -- Building step 4 under the one-way-shift proposal left three of jq's defining operators with
  nothing to do, which is a cost of the proposal rather than a refutation of it.
