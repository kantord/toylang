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
- [Each target constrains the design differently](each-target-constrains-the-design-differently.md)
  -- A target's speed constrains the design only if that target is meant to be fast; every
  target's correctness constrains it always.
- [A fourth backend found two rules three could not](a-fourth-backend-found-two-rules-three-could-not.md)
  -- Compiling to jq surfaced a scoping rule and an output rule that the other three all happened
  to satisfy, which is the argument for a structurally unlike target.
- [The backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md)
  -- Two prototype 1 bugs came from the emitted Lua rather than from toylang, and neither was
  visible anywhere in the front end.
- [Losing jaq's corpus means building the agreement harness](losing-jaqs-corpus-means-building-the-agreement-harness.md)
  -- Dropping the jaq fork also dropped a ready-made conformance suite, and cross-backend
  agreement testing is now toylang's problem.
- [Removing the effect layer makes map primitive](removing-the-effect-layer-makes-map-primitive.md)
  -- jq derives map from reflect-apply-reify, so a language without the layer cannot, and the
  operator that was free in the inspiration becomes a builtin here.
- [A pure value layer dissolves jq's iteration operators](a-pure-value-layer-dissolves-jqs-iteration-operators.md)
  -- Building step 4 under the one-way-shift proposal left three of jq's defining operators with
  nothing to do, which is a cost of the proposal rather than a refutation of it.
- [The lowering needs types the checker already computed](the-lowering-needs-types-the-checker-already-computed.md)
  -- Field access distributing over a Vec is the first construct whose lowering depends on a
  type, and the side table carrying it is a patch over a seam that wants merging.
- [Checked-only forms are a class, not a lambda rule](checked-only-forms-are-a-class-not-a-lambda-rule.md)
  -- Two more expressions turned up that can only be checked and never synthesised, so the
  annotation rule is about a class rather than about lambdas.
- [jq's item-wise access is the effect layer wearing brackets](jqs-item-wise-access-is-the-effect-layer-wearing-brackets.md)
  -- Executed edge cases show jq's brackets do not make access item-wise, the stream does, so the
  operator cannot be borrowed without the layer.
- [SoA is cheap until something wants a whole element](soa-is-cheap-until-something-wants-a-whole-element.md)
  -- Struct of arrays cost almost nothing because no operator extracts one element from a Vec,
  and the one place that needs a whole element turned out to be printing.
- [A type you can declare but cannot build](a-type-you-can-declare-but-cannot-build.md)
  -- Record types exist in the annotation grammar with no expression that produces one, so the
  only record a program can hold arrives from input.
- [Track an incomplete backend as a shrinking list](track-an-incomplete-backend-as-a-shrinking-list.md)
  -- A partial backend cannot join an agreement harness without softening it, and leaving it out
  is a silent skip, so snapshot what it cannot do and require that list to shrink.
- [Merging passes turns redundant traversals into bugs](merging-passes-turns-redundant-traversals-into-bugs.md)
  -- A double traversal that was pure waste while the checker only asked questions became a
  correctness bug the moment the checker also allocated.
