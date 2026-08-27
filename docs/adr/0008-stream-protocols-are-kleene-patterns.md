---
status: accepted
---

# Stream protocols are Kleene patterns, not a second stream type

Follows [Stream is the effect layer, typed](0001-stream-is-the-effect-layer-typed.md). When
streams carry protocol structure -- a fixed set of messages, a closing message, several ways
to end -- that structure is expressed with the sequence-pattern algebra draft.md's Q4 already
sketches (regular expressions over types: `Star`, `Seq`, `Alt`, with the empty pattern as
`Seq`'s unit), applied in effect position. No new stream primitive exists for any of it:

- `Stream<T>` is `Star<T>` in the effect layer; today's streams are the one-symbol star.
- Kleene plus, a provably nonempty stream, is `Seq<T, Stream<T>>` -- the base-functor
  head-plus-remainder shape.
- A closing message is `Seq<Star<T>, E>`; a payload-free end is the empty pattern, which by
  `Seq`'s unit law collapses `Seq<Star<T>, empty>` back into `Star<T>`, so the plain and
  protocol-carrying stream are one family, not a hierarchy.
- Several ways to end is `Alt` in the terminal position, e.g.
  `Seq<Star<T>, Alt<Eof, ParseError>>` -- which also gives mid-stream failure a typed home,
  and makes "errors are terminal" structural: after the end message the pattern admits no
  more items, a promise Rust's `Iterator<Item = Result<T, E>>` idiom cannot make.
- Consuming one item is the pattern's derivative; end-of-stream is the nullable match arm,
  the same information Rust's `next() -> Option` carries in `None`, moved from a sentinel
  value into a match arm.

One soundness condition keeps this compatible with the second-class stream decision:
**`Stream` (and any pattern containing one) never appears under a value constructor, and may
appear freely under pattern constructors.** Matching a pattern only continues the pipeline;
the remainder is bound linearly and never held. Without this rule, `Seq<T, Stream<T>>` is a
storable head-plus-rest pair and first-class streams return through the back door.

## Considered options

- A primitive `LowLevelStream<T, E>` (Rust-generator-style `Yield`/`Return`, Haskell
  streaming's `Stream (Of a) m r`), with plain streams as the trivial-`E` case. Rejected: the
  unit law makes the trivial case identical to `Star<T>`, so the hierarchy has one level; and
  the mandatory end slot forces inventing a `Unit` type, which this design has twice declined
  to do (`jsonlines` got no result type instead; `Lines` was made unprintable instead).
- Per-item fallibility, `Stream<Result<T, E>>`. Rejected: it permits an error followed by
  more items; the terminal-position `Alt` forbids that by shape.
- Renaming the star (`Stream` reserved for the protocol-carrying form). Rejected: every
  source that exists produces the empty-ended star, and richer shapes get user-space names
  through the type aliases the language already has.

## What this does not decide

Q4 stays open on everything that blocks implementation: union types (Q25), how tags are
represented at runtime (Q29), the matcher surface (Q27), and whether patterns spell inside
the constructor (`Stream<Plus<T>>`) or as outer combinators (`Seq<T, Stream<T>>`). No source
produces a non-star pattern yet, and a pattern type is only inhabited once a source does.
