# The best string type we can imagine: a spike

Commissioned by kantord/toylang#100 (oddities round, 2026-08-30): survey the ideal `Str`,
fully JSON-compliant, with internals that are simple, elegant, and performant, "just like we
did with the Int type". Nothing here compiles; the footprint is this file.

## What the Int precedent actually was

[Int is 32 bits and wraps](../docs/adr/0006-int-is-32-bits-and-wraps.md) was not a
representation choice. It was a contract choice: pick the observable behavior every backend
can carry *exactly* and *cheaply*, write it down as law, and refuse at the edges any value the
type cannot hold, so no two backends ever get the chance to disagree about it. The
representation (int32, `| 0`, wrapping helpers) then followed per backend, different in each,
identical in what it observes.

Strings need the same move, with one difference that changes the shape of the answer. `Int`
had one representation available in every target. Strings have three native models across the
seven backends: UTF-8 bytes (Go, Lua, Rust, native), UTF-16 code units (JavaScript), and
codepoint sequences (Python, and jq as observed through `explode`). No repo-wide
representation can be pinned, only the observations. So the "ideal Str" question decomposes
into: what is the contract, which stance on JSON gives it, and what does each candidate
internal model cost where we own one.

## The contract Str already has

Nobody has written it down as an ADR, but the corpus and the reference already enforce a
complete contract:

- A `Str` behaves as a finite immutable sequence of Unicode scalar values. The only
  decomposition is [`chars`](../docs/reference/builtins/chars.md), `Str -> Vec<Char>` by
  scalar value on every backend, and a [`Char`](../docs/reference/types/char.md) is never a
  surrogate half.
- Ordering is codepoint order, pinned as language law by #26 and stated in
  [the comparisons reference](../docs/reference/operators/comparison.md), including on the one
  backend whose native `<` disagrees
  ([codepoint order is not UTF-16 order](../research-log/codepoint-order-is-not-utf-16-order.md)).
- Concatenation (`+`), equality, and nothing else.
  [No length, no indexing, no splitting](../docs/reference/types/str.md): `extent` does not
  apply. This absence is load-bearing for everything below, because it means the language has
  never promised O(1) anything about a string's interior.
- The wire form is a JSON string, through one shared gate: `run_on` parses input with
  `serde_json`, validates, and re-serializes, so backends mostly receive normalized bytes
  rather than the caller's text
  ([unescapable control bytes are the crack in the re-serialization gate](../research-log/unescapable-control-bytes-are-the-crack-in-the-reserialization-gate.md)).

The recommendation at the end is mostly this list with an ADR number on it.

## "Fully JSON-compliant" has to pick a side on surrogates

The RFC 8259 grammar admits `"\ud800"` with no low surrogate: a lone surrogate escape is a
syntactically valid JSON string, and the RFC's own interoperability note (section 8.2) says
behavior on it is unpredictable. RFC 7493 (I-JSON) closes the hole by requiring strings to be
Unicode scalar values only. "Fully JSON-compliant" therefore means one of three things, and
the targets split across all three; verified directly against each toolchain (2026-08-30,
jq 1.8.2, Go and Node and CPython current):

- Round-trip untouched: Python (`json.loads('"\ud800"')` yields a lone surrogate and `dumps`
  re-emits it) and JavaScript (`JSON.parse` the same, one UTF-16 code unit).
- Replace: Go's `encoding/json` decodes it to U+FFFD, no error.
- Refuse: `serde_json` (which is the repo's gate; pinned as `refuses: true` by
  `tests/corpus/unpaired_surrogate_input.yaml`), jq 1.8.2 ("Invalid \uXXXX\uXXXX surrogate
  pair escape"), and the native runtime's own decoder (`tl_fail("unpaired surrogate")`,
  `runtime/toylang.c`).

Carrying the maximal reading, every RFC-8259-grammatical string, would mean a string type
that can hold unpaired surrogates. Rust's `String` cannot, by construction. Go's decoder
destroys the information before user code sees it. The only route is WTF-8 (Sapin's encoding,
what Rust uses for Windows `OsStr`): a repo-owned string type plus a hand-written JSON codec
in all seven targets, abandoning native strings everywhere, to faithfully carry data the spec
itself warns is unpredictable and that `chars()` could not decode into valid `Char`s anyway
(a `Char` is never a surrogate half, so `chars` would stop being total).

The right reading is the I-JSON one: a `Str` holds Unicode scalar values, and a lone
surrogate is refused at the gate, loudly, before any backend runs. That is what the corpus
already pins, it is the one stance all seven backends can implement without owning a string
type, and it is the same philosophy as the `Int` literal rule: a value the type cannot hold
should not exist long enough to disagree about. The honest cost is real and goes in the
recommendation: toylang cannot ingest every string a Python or JavaScript program can emit.

## The candidate internals

### UTF-8 bytes, iterated by scalar value

The Rust model, and already the model of four backends (Go, Lua, Rust, native) plus the wire
itself. Its elegance is one theorem: for valid UTF-8, byte order equals codepoint order. The
entire ordering law compiles to `memcmp` (`tl_str_cmp` in `runtime/toylang.c` is exactly
this), concatenation is two `memcpy`s, and input/output are copies rather than transcodes
because JSON travels as UTF-8. Scalar iteration is a ten-line decoder, of which the repo
already maintains two hand-rolled copies (C in the runtime, Lua in `emit_lua.rs`) and gets
the rest free (`range` in Go, `.chars()` in Rust).

The cost: no O(1) access to anything except byte length, which the language does not expose.
`chars()` is an O(n) decode, which is already its contract.

### UTF-16

What JavaScript gives natively, and nothing else does. The wire is UTF-8 in both directions,
so every edge is a transcode; native ordering diverges from the pinned law on any pair
straddling a surrogate (the corpus caught this; JS now carries a per-comparison codepoint
walk); and the representation can hold lone surrogates, values the wire gate refuses, so the
type would be wider than its own contract. The one thing UTF-16 buys, O(1) code-unit
indexing, indexes by a unit the language deliberately has no concept of. Nothing to adopt;
the JS backend inherits it and pays the shim.

### Codepoint arrays

Python's `str` is this (PEP 393: a flat array at 1, 2, or 4 bytes per codepoint, width chosen
per string), and jq's strings behave as this under `explode`. It buys O(1) *codepoint*
indexing, which at least indexes by the right unit, but the language has no indexing to spend
it on, and it costs up to 4x the memory of UTF-8 and a transcode at every wire edge. As a
repo-owned representation it buys nothing over UTF-8 bytes; as an inherited one (Python, jq)
it is harmless, since codepoint-elementwise comparison agrees with the ordering law by
definition.

### Ropes

A rope's case is concatenation-heavy workloads:
[`join` and `unlines`](../docs/reference/prelude/join.md) are recursive `+` in `prelude.toy`,
which on flat immutable strings is quadratic in total length. But the representation lever
exists in exactly one place, the native runtime; the other six backends' `Str` is the
target's own string type, and reimplementing strings in six languages to fix a prelude
function's asymptotics is the WTF-8 mistake with a different motive. The observed workloads
(the corpus, the Euler stream) use strings as labels and output lines, not as megabyte
buffers being built by accretion.

If join cost ever shows up on a real program, the cheap fix is a size class below ropes: make
`join`/`unlines` builtins over each target's native builder (`strings.Builder`,
`Array.prototype.join`, `str.join`, `table.concat`, `String::push_str` after a `reserve`, and
a one-pass sum-then-memcpy in C, which `tl_join_parts` in the runtime already does for
printing). Linear time, zero new representation. A rope is complexity that outlives its one
caller.

### Small-string optimization

Native runtime only, again. `tl_str` is a `{ptr, len}` pair behind a `malloc` per value;
SSO would inline short payloads into the handle and skip the allocation. The win exists for
workloads making many tiny strings, and no such workload has been observed or profiled here.
It also forces `tl_str` to move by value through every runtime signature that currently
passes pointers. Noted as the first lever if native string allocation ever profiles hot;
premature before that.

## What the seven backends natively give

| backend | native model | codepoint ordering | scalar iteration |
|---|---|---|---|
| Go | UTF-8 bytes | native `<` (byte order) | `range` decodes runes |
| JavaScript | UTF-16 code units | `tl_str_cmp` walks codepoints | string iterator + `codePointAt` |
| Python | codepoint array (PEP 393) | native `<` | `ord` per character |
| Lua | raw bytes | native `<` (byte order) | hand-rolled UTF-8 decoder |
| Rust | `String`, valid UTF-8 enforced | native `<` (byte order) | `.chars()` |
| jq | codepoint strings | native `<` | `explode` |
| native/LLVM | `tl_str` ptr+len, UTF-8 | `memcmp` | hand-rolled UTF-8 decoder |

The pattern worth naming: six of seven get the ordering law from their native comparison,
because byte order over valid UTF-8 and codepoint-elementwise order are both codepoint order.
Only the UTF-16 backend needs a shim, and only the two byte-blind targets (C, Lua) need a
hand-written decoder, which is the same ten lines twice. UTF-8 bytes compared by `memcmp` is
the fixed point the backends already orbit; the ideal representation was reached by six
backends without anyone choosing it.

## What chars, join, and the missing extent imply

`chars()` is total (every `Str` is scalar values, so decoding cannot fail) and one-way: there
is no `from_chars`. If the inverse ever lands, it is the encoder mirror, and every backend
has one (`String.fromCodePoint`, `chr`, `utf8.char`, `string([]rune)`, `char::from_u32`, and
the runtime's existing `tl_utf8_encode`), with one new obligation: refusing or preventing
surrogate-half `Char` values from encoding, which today cannot arise because `chars` never
produces one and `Char` has no literal.

`join` being quadratic is a fact about the prelude, not the representation, and the fix
ladder is in the ropes section above. On the native backend `chars()` also expands storage:
`Vec<Char>` lands in the SoA vec at 8 bytes per element, a 2-8x expansion of the string it
decoded. Fine at corpus scale; real for large texts.

`extent` not applying to `Str` is what keeps every candidate above honest. The moment the
language promises a string length or an index, it must pick the unit (byte, code unit,
codepoint, grapheme), and the UTF-16 gap the corpus closed for ordering
[reopens there independently](../research-log/codepoint-order-is-not-utf-16-order.md). The
current design's refusal to give `Str` dimensions is not a gap in the string type; it is the
string type.

## Wire constraints, edge by edge

`input`/`inputs`: one `serde_json` gate, then re-serialization. Lone surrogates refused
(pinned), key case exact (pinned), and the one class of content the gate cannot normalize
away, control characters with no named escape, now decodes correctly in the one backend that
parses strings by hand
([the re-serialization gate note](../research-log/unescapable-control-bytes-are-the-crack-in-the-reserialization-gate.md)).

Output: a top-level `Str` prints raw; a nested one prints JSON-quoted and escaped
([Str reference](../docs/reference/types/str.md)). Both directions of the wire are UTF-8, so
the UTF-8-bytes model is the only candidate with no transcode at either edge.

[`lines`](../docs/reference/sources/lines.md) is the unpinned edge. It hands over "the bytes
between terminators" with no JSON parsing, and nothing pins what happens when those bytes are
not valid UTF-8: the byte-string backends (Go, Lua, native) would carry them, Rust's line
reader would error, and what JavaScript's and Python's readers do to them (replace, refuse)
has not been proven per backend. This is the one place the "Str is Unicode scalar values"
contract can currently be violated by construction. Filed as kantord/toylang#102 rather than
decided here, since either answer changes observable behavior; the conservative candidate is
refusal at the edge, matching the gate and the `Int` literal rule.

## Recommendation

The ideal string type is the one the project has already converged on; what is missing is the
document and one pinned edge.

1. Write the ADR: a `Str` is a finite immutable sequence of Unicode scalar values; ordering
   is codepoint order; the wire form is an I-JSON string; a lone surrogate is refused at
   every edge. Every clause is already enforced by corpus or reference, so this is recording,
   not deciding, the same way ADR 0006 recorded Int.
2. Representation stays per-backend native, with UTF-8 bytes plus scalar-value iteration as
   the reference model that the native runtime implements and the hand-rolled decoders
   mirror. No rope, no SSO, no repo-owned string type in the six emitted targets.
3. Pin the `lines` edge (#102): decide and corpus-pin what a non-UTF-8 byte arriving as a
   line does on every backend.
4. If `join`/`unlines` ever profile hot on a real program, promote them to builtins over
   target-native builders. Ropes stay ruled out at this workload shape.

The honest costs of this position:

- JavaScript pays a codepoint walk on every `Str` ordering comparison, forever. Equality is
  unaffected.
- Refusing lone surrogates means some JSON that Python and JavaScript programs emit is
  refused at the gate. A pipeline fed such data fails loudly instead of processing it; that
  is the intended trade, and it is still a refusal a user will meet.
- `join` stays quadratic until a workload proves it matters, and the proof will arrive as
  someone's slow program.
- No O(1) length or indexing, ever, without reopening the per-backend unit question. This
  recommendation entrenches the absence deliberately.
