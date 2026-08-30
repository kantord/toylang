---
type: Reference
title: research-log index
description: Index of what building toylang taught us; every note is linked from here and from at least one sibling.
---

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
- [A statically typed target asks for the types the checker already has](a-statically-typed-target-asks-for-the-types-the-checker-already-has.md)
  -- Go needs a declared name for every record and the element type spelled at every level of
  distribution, and the checker had computed all of it already.
- [Backends can agree and still be wrong](backends-can-agree-and-still-be-wrong.md)
  -- An out-of-range Int literal printed identically on four backends, because each avoided the
  rule by its own accident rather than by following it.
- [One invariant, three independent construction sites](one-invariant-three-independent-construction-sites.md)
  -- A Vec of records is one column per field on native, and field access, map, and a Vec
  literal each independently violated it, none of the earlier fixes implying the next was safe.
- [Streaming input is pull, verified against jq itself](streaming-input-is-pull-verified-against-jq-itself.md)
  -- Every mature abstraction that stops a producer from outrunning its consumer turns out to be
  pull, including jq, which decided the model with no new machinery needed for backpressure.
- [A sixth instance of the backend having rules the checker does not](a-sixth-backend-rule-the-checker-did-not-know.md)
  -- Go's default line scanner strips a carriage return the other five backends leave alone, and
  mlua's embedded Lua reads the real process stdin directly with no interception needed.
- [A backend that finds nothing is evidence only if it is different](a-backend-that-finds-nothing-is-evidence-only-if-it-is-different.md)
  -- Python ported 68 programs with no front-end change, which is worth something only because it
  fails unlike the five already there.
- [A config field that is ignored is a check that did not run](a-config-field-that-is-ignored-is-a-check-that-did-not-run.md)
  -- Letting corpus cases ask for extra checks created a new way to fail silently, since a
  misspelt key asks for nothing and looks exactly like a case that passes.
- [Merging passes turns redundant traversals into bugs](merging-passes-turns-redundant-traversals-into-bugs.md)
  -- A double traversal that was pure waste while the checker only asked questions became a
  correctness bug the moment the checker also allocated.
- [Juxtaposition is unsafe at any undelimited boundary](juxtaposition-is-unsafe-at-any-undelimited-boundary.md)
  -- Adding parenless function application broke a program with no application in it, because two
  unrelated expressions had always sat adjacent at one spot the grammar never bothered to
  delimit.
- [winnow replaced the tokenizer, not the grammar](winnow-replaced-the-tokenizer-not-the-grammar.md)
  -- Porting the hand-rolled lexer and parser onto winnow kept every dispatch decision and error
  message exactly as written, because the library's real value here was position tracking and
  character-level primitives, not its combinators.
- [Named functions kept an open question open](named-functions-kept-an-open-question-open.md) --
  extent, concat, and tail were spelled as named builtins rather than as an operator overload or
  a new Spec, specifically so adding them would not force an answer to Q2, which is still open.
- [Vec of enum falls into the boxed default nobody chose](vec-of-enum-falls-into-the-boxed-default-nobody-chose.md)
  -- Boxing a scalar enum as a tag-plus-payload record made Vec<enum> run on native with zero
  added code, so step 4's open layout choice already has a de facto answer that no test pins and
  nobody decided.
- [A map body cannot infer from what consumes it](a-map-body-cannot-infer-from-what-consumes-it.md)
  -- expect() threads an expected type into exactly one form, Expr::Input; a map body is always
  synth'd bottom-up, so a polymorphic parse(s) -> T builtin has nothing to resolve T against
  inside map(parse(.)), the same hole an empty [] literal falls into.
- [Codepoint order is not UTF-16 order](codepoint-order-is-not-utf-16-order.md) -- JavaScript's
  native `<` on strings compares UTF-16 code units, which disagrees with the other six backends'
  codepoint order on any pair straddling a surrogate pair.
- [Unescapable control bytes are the crack in the re-serialization gate](unescapable-control-bytes-are-the-crack-in-the-reserialization-gate.md)
  -- Input is validated once against serde_json and re-serialized before any backend runs, which
  hides six backends' JSON-parsing differences except for the one class of character that gate
  cannot normalize away.
- [A match evaluated its subject even when no arm read it](a-match-evaluated-its-subject-even-when-no-arm-read-it.md)
  -- Native's match codegen called self.expr(subject) unconditionally, forcing a whole-record read
  out of a struct-of-arrays cursor for a guard chain that never touches the subject value.
- [Borrowing the host's null borrowed its conflations](borrowing-the-hosts-null-borrowed-its-conflations.md)
  -- Making absence tagged (#62) required changing only the four backends that had mapped Opt
  onto a host null-ish value; the three that had built their own representation were already
  tagged and needed nothing.
- [Reorder found a route around the layout it did not reconcile](reorder-found-a-route-around-the-layout-it-did-not-reconcile.md)
  -- Reaching inside an Opt payload for record-reorder (#66) needed a way to rebuild a present
  value, not a way to match one, so it got its own node instead of waiting on the Match/Opt
  layout reconciliation the previous note left open.
- [A recursive type is where inline codegen needs a name](a-recursive-type-is-where-inline-codegen-needs-a-name.md)
  -- Every backend writes its printers and parsers by expanding a type inline, which works until
  the type contains itself; the fix is the same in all seven, and the placeholder standing in for
  the recursion had let each of them answer wrongly rather than fail.
