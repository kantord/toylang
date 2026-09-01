# Erlang as a target: what its semantics would actually demand

Desk research for issue #163, which floats Erlang as a candidate backend whose
point would be to stress test the design. The question here is narrower than "would
this be nice to have". The project's admission test for a backend
([ADR 0002](../docs/adr/0002-backends-as-falsifiers.md), and the research note
[a backend that finds nothing is evidence only if it is different](../research-log/a-backend-that-finds-nothing-is-evidence-only-if-it-is-different.md))
is which axis a candidate is unlike the existing ones on. Everything below is a
documented-semantics comparison; no Erlang toolchain was consulted or run.

The short version of the finding: Erlang is unlike every existing backend on exactly
one axis, concurrency and message passing, and toylang has no construct that would
exercise that axis. On every axis the language actually uses, Erlang duplicates an
existing witness. As a shipped backend it fails the admission test. As a *thought
experiment* against the open questions, it is the most opinionated thing available,
and two of its features (selective receive, process-as-stream) sit directly on open
design questions. So the stress test is worth doing, but it is a research exercise,
not a backend.

## Erlang's model, in the vocabulary the design already uses

Erlang is a dynamically typed, single-assignment language whose concurrency is the
organizing principle rather than a library. Four facts matter here, each of which is
a claim about toylang vocabulary.

**Terms are immutable, nested data.** An Erlang term is an atom, integer, float,
binary, tuple, or list, composed arbitrarily. There is no mutation; a variable binds
once per clause and then names a value forever. That is toylang's value model minus
the JSON shapes: an atom is a bare-string enum variant or a string, a tuple is a
record or a single-key enum wrapper, a list is a `Vec`, a binary is a string. The
nested term is exactly the canonical JSON value, rendered in Erlang's own notation.

**Pattern matching is the central operation, and it is single-assignment.** Erlang
dispatches on shape everywhere: function clauses are tried in order and the first
whose head matches wins, `case` and `receive` do the same. A match binds each
variable at most once, compares literals, and walks nested structure; there is no
backtracking and no unification of one match's bindings with another's. This is
toylang's ordered arm list with the lens-style decode left out.

**Processes communicate by message passing.** A process has a mailbox. It can
`send` a term to another process's identifier and `receive` messages, and here is
the feature with no toylang analogue: `receive` is *selective*. A process writes a
list of message patterns and the runtime scans the mailbox for the first message
matching any of them; messages that match nothing are left in place for a later
receive. Order of arrival is not order of consumption.

**There is no type system and no effect system.** Dialyzer infers success typings
as an optional analysis, but the language itself has no compile-time types, no
cardinality, and no notion of an effect. Sending, spawning, and I/O are ordinary
operations with no tracking. A missing clause or a message no pattern matches is a
runtime `function_clause` or `case_clause` error, not a check.

## Where toylang maps cleanly

The mapping is clean exactly where Erlang duplicates a witness the project already
has.

**Ordered arm matching is the same machinery as Erlang clause matching.** Toylang's
matcher is first-match-wins over an ordered arm list with guards, an `or`
composition, and a `partial` flag that types an honest chain as `Opt`
([guides/matching](../docs/guides/matching.md), [pattern matching is decoding](../draft.md#pattern-matching-is-decoding)).
Erlang's clause and `receive` matching is the same ordered, single-pass, no-unification
walk. This confirms two decisions rather than challenging them. The no-unification
stance ([Q28](../plans/questions.md#q28-does-deep-matching-need-cross-match-unification-of-logic-variables))
is exactly what Erlang does: `as`-style binding within one arm exists, cross-match
Prolog unification does not. And the `partial` flag is the answer to a hole Erlang
has: toylang types a possibly-total chain instead of deferring to a runtime
`case_clause`. That is toylang doing better, not differently.

**The value model is a near-translation.** Every toylang value is canonical JSON
and every backend already renders that. Erlang terms are the same nested immutable
data. On the value axis Erlang is another copy of the JSON-native witnesses, with a
different syntax. The enums of [ADR 0009](../docs/adr/0009-enums-are-json-native-single-key-wrappers.md)
lower to Erlang exactly as they lower to the others: a unit variant is an atom, a
payload variant is a tuple of tag plus payload.

**Immutability is shared, and the cell story stays intact.** Erlang has no mutation
at all. toylang's `cell` is a deliberate, checked exception to an otherwise
immutable model. Erlang is the existence proof that the model works without the
cell, which supports the direction but changes nothing: no toylang program uses
`cell` for anything Erlang would force to be untracked mutable state.

## Where it does not map

**Selective receive is not a stream.** Toylang's effect layer
([ADR 0001](../docs/adr/0001-stream-is-the-effect-layer-typed.md)) is one linear,
in-order, exactly-once chain: born at a source, consumed once, dead at a sink. An
Erlang mailbox is a holdable, reorderable, partially consumed source. A selective
receive does not consume the front of a queue; it scans a bag for a shape and
leaves the rest. This is the one place Erlang is genuinely unlike everything the
project has built, and it is also the one place toylang has no construct to receive
it. There is no toylang expression that produces a mailbox, and no way to express
"consume these messages, leave those for later" in the linear stream model. So
Erlang does not falsify the stream rules; it simply has no analogue to feed into
them.

**There is no target-level type to be a witness against.** The checked-cardinality
apparatus, `Stream<T>`'s exactly-once rule, `Opt` for partial chains, the enum
exhaustiveness check: none of these has an Erlang counterpart, so none of them gets
stress-tested by emitting to Erlang. A toylang type error that the Go and Rust
backends would catch with a compiler would slip through to a runtime clause error on
Erlang. As a correctness witness Erlang is weaker than Rust, which already provides
the statically-typed, real-sum-type, native-match witness that Erlang would most
resemble.

**Effects have no spelling to erase.** toylang's effect layer is typed, which is
the point of the streams decision. Erlang's effects (send, spawn, I/O) are
untracked and interleaved freely with pure computation. Emitting a toylang program
to Erlang erases the type-level effect information and replaces it with nothing, so
the thing the streams decision exists to check cannot be observed on this target.

## Three candidate framings

The brief asks how the cardinality/effect model would map onto actor semantics.
Here are the three ways that question can be taken, and what each one concludes.

**Framing one: Erlang as an eighth backend.** Add `Erlang` to the `Backend` enum and
an `emit_erl.rs` that lowers every feature like the others. This is the framing the
issue most literally proposes, and it is the one that fails the admission test. The
test is which axis a candidate is unlike the existing ones on, and the only axis
Erlang differs on is actors, which toylang programs never use, so the eighth emitter
would never emit a process, a send, or a receive. On value, matching, streaming, and
immutability it duplicates existing witnesses. The cost, an eighth hand-written
emitter and an eighth set of output rules, buys no falsification. Reject.

**Framing two: Erlang's mailbox as a stress test for the stream model.** Take the
actor's selective receive seriously as a counterexample to the one-way, in-order,
exactly-once stream. This is where Erlang earns its keep, because it sharpens an
open question. The out-of-order consumption is a concrete instance of
[Q4's ordering half](../plans/questions.md#q4-can-the-type-express-ordering-over-heterogeneous-streams),
which the Kleene-pattern algebra of [ADR 0008](../docs/adr/0008-stream-protocols-are-kleene-patterns.md)
deliberately does not cover (its patterns are consumed in arrival order). Erlang shows
what "consume in a different order than arrival" costs and why a linear language says
no to it: selective receive is the reason a mailbox can grow without bound and why
unmatched messages can starve. That is a real finding for the design, but it is a
finding about *not* building selective receive, and it does not need Erlang as a
backend to record it.

**Framing three: Erlang's process-as-stream as a live counterexample to
[ADR 0001](../docs/adr/0001-stream-is-the-effect-layer-typed.md).** The idiomatic
Erlang stream is a process you send messages to; it is a value, a mailbox you can
hold, reorder, and selectively read. ADR 0001 rejected first-class streams as "the
one irreversible option", a held value of genuinely unknown extent. Erlang is a
working language built on exactly that held, unknown-extent value, which makes it
the strongest available argument against the rejection. But it is an argument, not a
requirement: Erlang pays for first-class streams with a runtime that scans mailboxes
and a total absence of the checked linearity the streams decision exists to provide.
The tension is worth naming in the ADR's "considered options" record, not worth
reopening.

## Recommendation

Do not add Erlang as an eighth backend. Its one distinct axis, concurrency and
message passing, is absent from the toylang language, so an emitter would never
exercise it, and on every exercised axis it duplicates a witness already present
(pattern matching and statically-typed terms duplicate Rust; the JSON value model
duplicates Lua, JS, and Python; the streaming loop duplicates jq and the others).
By the project's own admission test, which the research log states as "not whether
it would be useful to have, but which axis it is unlike the others on", Erlang fails.

Do use Erlang as a design stress test in the cheaper sense: a documented comparison
against the open questions it is most opinionated about. It is the best available
probe for two open items. It gives [Q4](../plans/questions.md#q4-can-the-type-express-ordering-over-heterogeneous-streams)
a concrete picture of out-of-order consumption and why the Kleene-pattern shape
declines it, and it gives [Q35](../plans/questions.md#q35-what-are-stdout-and-stderr-and-does-a-program-write-or-return)
the strongest version of the "output is an effect, not a value" answer, since Erlang
has many processes each writing independently and no single program value at all.

If the language ever grows concurrency, revisit Erlang as a backend: it would then
be the obvious first witness on a new axis, the way jq was the first witness on
generative streaming. Until then, the honest summary is that Erlang is a question
the design should answer in prose, not a target it should compile to.

If issue #163 is to be closed rather than left as a backlog idea, the recommendation
is to record this analysis and close it as "considered, not a backend, see
plans/erlang-target-research.md". The one condition that would reopen it, added
concurrency, is not on any board.
