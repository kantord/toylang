---
status: accepted
---

# Str is a sequence of Unicode scalar values, ordered by codepoint

Recorded 2026-09-07, after the fact:the contract was already enforced by the corpus and
the reference before anything here was decided. The string-type spike
([plans/string-type-spike.md](../../plans/string-type-spike.md)) surveyed the ideal `Str` across all
seven backends and found the language had converged on one contract;this ADR records it the way
[ADR 0006](0006-int-is-32-bits-and-wraps.md) recorded Int:a contract choice, written
down as law, with values the type cannot hold refused at the edges so no two backends ever get the
chance to disagree.

A `Str` behaves as a finite immutable sequence of Unicode scalar values. The only
decomposition is [`chars`](../reference/builtins/chars.md), `Str -> Vec<Char>` by scalar value on
every backend,and a [`Char`](../reference/types/char.md) is never a surrogate half. There is
no string length, splitting, or indexing -- a `Str` has no dimensions (`extent` does not
apply), so the language has never promised O(1) access to a string's interior. Concatenation
(`+`) and equality are the only other operations.

Ordering (`<` and friends) compares by Unicode codepoint, pinned as language law across
every backend including the JavaScript target, whose native `<` compares UTF-16 code units
instead, so it walks codepoints rather than using `<` directly
([codepoint order is not UTF-16 order](../../research-log/codepoint-order-is-not-utf-16-order.md);
pinned by `tests/corpus/str_ordering_codepoint.yaml`).

The wire form is an I-JSON (RFC 7493) string:a `Str` holds Unicode scalar values only,
and a lone surrogate is refused at every edge, loudly, before any backend runs. `input` and
`inputs` parse through one shared `serde_json` gate and re-serialize
([unescapable control bytes are the crack in the re-serialization gate](../../research-log/unescapable-control-bytes-are-the-crack-in-the-reserialization-gate.md)),
so every backend receives normalized bytes rather than the caller's text,and the gate itself
refuses a lone surrogate (`tests/corpus/unpaired_surrogate_input.yaml`). A `Char` has no wire
form at all, refused both as `input`'s type and as a printed result
([`Char` reference](../reference/types/char.md)).

The choice is the I-JSON reading of "fully JSON-compliant", not the maximal one. The maximal
reading -- every RFC 8259-grammatical string, including `"\ud800"` -- would force a string
type that can hold unpaired surrogates:Rust's `String` cannot, by construction; Go's decoder
destroys the information before user code sees it;and `chars` could not decode them into valid
`Char`s anyway. The honest cost is real and accepted:toylang cannot ingest every string a
Python or JavaScript program can emit,and a pipeline fed such data fails loudly instead of
processing it. The full survey of the candidate internals lives in the spike, not here.

The `lines` edge stays unpinned:what a non-UTF-8 byte arriving as a line does on each
backend is still an open question,filed as kantord/toylang#102. It is the one place the
"scalar values only" contract could currently be violated by construction.