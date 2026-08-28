---
type: Lesson
calendar:
  - 2026-08-28
title: Unescapable control bytes are the crack in the re-serialization gate
description: Input is validated once against serde_json and re-serialized before any backend runs, which hides six backends' JSON-parsing differences except for the one class of character that gate cannot normalize away.
tags:
  - strings
  - backends
  - native
  - input
timestamp: 2026-08-28T00:00:00Z
---

`run_on` (`src/lib.rs`) parses `input`/`inputs` with `serde_json` once, validates the value
against the declared type, and hands every backend the re-serialized bytes rather than the
caller's original text. The comment there is explicit about why: "generated code cannot be
trusted to reimplement `input::validate` correctly in six different target languages." What that
same re-serialization does as a side effect, not by design, is normalize away most of what a
backend's own JSON decoder would otherwise have to get right independently -- which is
[the backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md)
turned inside out: instead of a backend's own rules surfacing through disagreement, they mostly
never fire at all.

**Two suspected gaps that never reach a backend.** A JSON key differing only in case from a
declared field (`{"Name": "ada"}` against a field `name`) and a lone, unpaired surrogate escape
(`"\ud800"` with no low surrogate) both looked like plausible cross-backend divergences worth
proving -- Go's `encoding/json` falls back to case-insensitive struct-field matching when no
exact key exists, and six backends each decode `\u` escapes with their own logic. Neither ever
reaches a backend: `input::validate`'s `map.get(name)` is an exact, case-sensitive lookup, and
`serde_json::from_str` itself rejects a lone surrogate as malformed JSON, both inside the one
shared gate every backend sits behind. `tests/corpus/field_key_case_is_significant.yaml` and
`tests/corpus/unpaired_surrogate_input.yaml` pin this as `refuses: true` -- confirmed by running
all seven backends against both inputs directly before writing either case down, the same
verify-before-recording discipline
[a test that cannot fail is worse than no test](a-test-that-cannot-fail-is-worse-than-no-test.md)
asks for.

**The one crack: a control byte with no named JSON escape.** `serde_json`'s writer emits raw
UTF-8 for every character it can -- an accented letter and an astral emoji both round-trip
through re-serialization as literal UTF-8 bytes, never as a `\u` escape, which is why
`tests/corpus/unicode_input.yaml` already passed on every backend including native. But JSON
forbids an unescaped control byte (`U+0000`-`U+001F`) in a string, so when the value contains one
without a named shorthand (`\b \f \n \r \t` cover five of them; NUL, `U+0001`, and the rest have
none), `serde_json` is forced to write it as `\u00XX` -- and that generic escape is the one thing
the gate cannot make disappear. `runtime/toylang.c`'s hand-rolled `tl_parse_string` had no `case
'u'` at all, so it failed outright: `unsupported escape at input.tag` on a NUL byte, while the
other six backends (five with mature JSON libraries, and Lua, which never parses JSON itself --
`mlua` receives values `input::to_lua` already converted from the same `serde_json::Value`)
printed the byte correctly. Go, Python, JS and Rust's libraries had never been asked to prove
this; only the two paths this project wrote by hand (native's C decoder, and the Rust gate
itself) were ever actually at risk.

**The fix.** Added `\u` escape decoding to `tl_parse_string`, including surrogate-pair combining
into one codepoint before UTF-8-encoding it -- a complete decoder, not just enough to pass
`\u00XX`, even though the surrogate-pair branch is currently dead code in this specific pipeline
(the gate never re-emits a surrogate escape for a valid character; it always prefers raw UTF-8).
Pinned with `tests/corpus/control_char_escapes.yaml`, one string carrying all five named escapes
plus a bare space, run through `input` so every backend's real decoder -- what few of them have
one -- is what answers.

Open: the same shape of question applies to `lines`, which reads raw text with no JSON parsing
at all, so nothing here says anything about a control byte arriving that way.

Found in the same session as
[codepoint order is not UTF-16 order](codepoint-order-is-not-utf-16-order.md), the other shape
this kind of gap takes: a backend's own encoder disagreeing on a comparison, rather than its
decoder having a rule nothing before it had tested.
