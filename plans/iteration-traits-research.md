# Iteration traits: what one definition would have to promise

Research spike for [`iteration-traits-research`](board.yaml), feeding the
[`iteration-traits-design`](board.yaml) decide row. The trigger is a maintainer note on `max`
(2026-09-06): "max is defined separately for Vec and Stream today, but both only need a simple
linear iteration (sync or async, any order) to compute it." Spike whether an iteration trait (or
a family of them) would let one function definition serve both Vec and Stream, and whether it
should split by the property a function actually promises rather than being one monolithic
"iterable" contract. No `src/` change is expected.

## The premise needs correcting first

`max` is not defined for both Vec and Stream. It is Vec-only. `max_call`
([`src/check/mod.rs:2673`](../src/check/mod.rs)) keys on `arg.ty.elem()`, and `elem`
([`src/ty.rs:157`](../src/ty.rs)) returns `Some` only for `Type::Vec`, never for
`Type::Stream`. The reference doc ([`max.md`](../docs/reference/builtins/max.md)) documents only
`Vec<Int>` and `Vec<Int64>`, and the annotation the note came from says the same. There is no
Stream `max` to unify: no stream reducer of any kind exists. `streams.md:86` records "there are
no stream reducers in this plan," and [`draft.md:1727`](../draft.md) repeats it -- "a fold over a
stream is real future work, not a typing tweak." So the trait is being asked to unify a pair that
does not exist yet, which changes what it has to do. The question is not "how do we serve both
existing definitions," but "when stream reduction is built, what shape lets it share with the Vec
path."

The "sync or async" half of the note also does not survive contact with the design. Stream here
is not an async/await layer; it is a linear pull iterator (the pull decision, with fan-out and
concurrency named as non-goals in [`streams.md:88`](../plans/streams.md)). Arrival timing is the
source's business and nothing else, so "sync or async, any order" reduces to "arrival order is
unconstrained," which is exactly what `max`'s commutativity already grants. There is no second
dimension to split on there.

## What the code actually splits today

The one precedent of a single definition serving both Vec and Stream is the mappers. `select` and
`map` use `mapper_elem` ([`src/check/mod.rs:916`](../src/check/mod.rs)), which matches
`Type::Vec(t) | Type::Stream(t)`, and the comment there states the split explicitly: `mapper_elem`
is "deliberately not `Type::elem`, which stays Vec-only so the reducers (`length`, `jsonlines`
today) keep refusing a stream." The mappers are per-element and produce the same `Kind::Map` /
`Kind::Select` over either source, so the backends emit a loop regardless. The element type is
what the two share.

Everything blocking is Vec-only, and all through the same `elem()` gate: the reducers `sum`
([`mod.rs:2647`](../src/check/mod.rs)) and `max` ([`mod.rs:2673`](../src/check/mod.rs)), the
order/whole-collection operations `sort` ([`mod.rs:2589`](../src/check/mod.rs)), `reverse`
([`mod.rs:2619`](../src/check/mod.rs)), `sort_by` ([`mod.rs:2702`](../src/check/mod.rs)),
`max_by` ([`mod.rs:2741`](../src/check/mod.rs)), and `length` ([`mod.rs:2498`](../src/check/mod.rs))
and `first` ([`mod.rs:2779`](../src/check/mod.rs)). That uniformity is the
deliberate shape, not an accident: `elem` staying Vec-only is what makes a reducer refuse a
stream. The full set of reducers today is exactly two -- `sum` and `max` -- because the design cut
`min` and `product` on the same grounds as `reducible` records ([`mod.rs:2640`](../src/check/mod.rs)).

## Why the reducers being Vec-only is not a trait gap

The blocker is not a missing abstraction. `max` refuses a stream because no stream reducer
exists and `Stream` has unknown, unbounded extent ([`offload.rs:6`](../src/offload.rs)), so there
is nothing for a stream `max` to return incrementally against a whole-collection promise. Making
`max` serve a stream is the decision to add stream reduction ([`draft.md:1727`](../draft.md)),
and the iteration-trait question is subordinate to it. A trait cannot conjure a stream `max`; it
can only express the shared shape once reduction over a stream is allowed.

## Could one definition serve both?

Only through a generic function with a trait bound -- a fold over some `Iterable`. And that is
precisely the feature the prior trait research deferred. [`trait-interface-research.md`](trait-interface-research.md)
scoped a monomorphic dispatch table (a `trait`, per-type `impl`s, static dispatch by
`(trait, type)`) and put the polymorphic builtins out of scope: "Making the polymorphic builtins
(`length`, `sum`, `flatten`, ...) into trait instances: that is the generic-functions landing, not
the monomorphic cut." A dispatch table dispatches by a concrete type the checker already has; it
cannot express "any type with this iteration capability." So "one definition serves both Vec and
Stream" is the generic-functions feature, not the dispatch-table feature. Building an iteration
trait as a monomorphic table now would either do nothing (no type is abstract yet) or force the
generic-function step early.

## Is the property split the right shape?

The note's instinct is sound: a single monolithic "iterable" contract is simultaneously too
strong for `max` and too weak for `sort`. But the axes the note points at are already partly
drawn by the type system, and the genuinely new axis is a different one.

- `max`, `sum`: associative and commutative fold; arrival order unconstrained.
- `any`, `all`, `length`: order-insensitive, but they are Vec-only today too and would need the
  same stream-fold step as `max`.
- `first`: order-sensitive (the earliest item) but does not need the whole collection.
- `sort`, `reverse`: need the whole collection and a fixed order (blocking).

The blocking line -- whole-collection versus incremental -- is already `Vec` versus `Stream` in
the type grammar. So a separate trait family that re-encodes "blocking" would be redundant. The
axis the type system does not already carry is the one between reducers that are order-free and
reducers that are not: `max` and `sum` fold in any order, while a hypothetical `first` needs the
first item and a running maximum that depends on arrival order would not. That is the split worth
naming -- promise only the properties a fold needs (whether it is associative, and whether it is
commutative), rather than one `Iterable` that over-promises order.

## Recommendation

Do not add an iteration trait as a monomorphic dispatch table now. There is nothing to unify: no
stream `max` exists, the reducer family is two functions, and the "one definition serves both"
shape is the deferred generic-functions landing that `trait-interface-research` already scoped
out of the monomorphic cut. Record the property-split principle as the shape the fold trait
should take when stream reduction is designed: promise only what the fold needs (associativity,
and commutativity where the reduction is order-free), so `max`'s "termination, order-free" and a
hypothetical ordered fold differ by trait rather than by one bloated `Iterable`. The blocking
distinction stays where it is, in `Vec` versus `Stream`.

## Open questions for the design row

- Whether stream reducers should exist at all; `draft.md:1727` defers them, and the trait
  question is downstream of that call.
- Whether "arrival order free" is a separate commutative-fold trait or a property the design
  chooses per builtin, and whether a float `sum`'s non-commutativity under rounding is a reason
  to split associativity from commutativity.
- Whether the Vec/Stream line already carries the blocking distinction tightly enough that a
  separate iteration-trait family adds nothing until generic functions land.
