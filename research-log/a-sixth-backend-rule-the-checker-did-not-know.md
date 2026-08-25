---
type: Note
calendar:
  - 2026-08-25
title: A sixth instance of the backend having rules the checker does not
description: Go's default line scanner strips a carriage return the other five backends leave alone, and mlua's embedded Lua reads the real process stdin directly with no interception needed -- two more facts about a target found by testing it rather than assuming it.
tags:
  - streams
  - backends
  - go
  - lua
timestamp: 2026-08-25T00:00:00Z
---

Reading stdin line by line meant picking a rule -- split on `\n` only, strip it, leave a bare
`\r` alone, still yield a final line with no trailing newline -- and then finding that no single
backend's own native mechanism was trusted to already match it without checking.

**Go's default `bufio.Scanner` disagreed.** `ScanLines`, the stdlib's own split function, strips
a trailing `\r` along with the `\n` -- verified directly, `"a\r\n"` in gave `"a"` out, not
`"a\r"`. Every other backend (`jq -R`, Python's raw stdin iteration, `getline`) left it alone.
The fix is `ScanLines` copied with its one `dropCR` line removed, rather than the stdlib default,
because the rule needed to hold across all six and Go's own default was the one backend that did
not.

**`wc -l` was the negative example, and confirming it mattered.** It counts newline characters
rather than lines, so `printf 'a\nb'` (no trailing newline) reports 1 rather than 2 -- verified
directly rather than assumed from having heard of the gotcha before, which is what turned it from
folklore into a rule with a checked-against fact: the final line, however it is terminated, is
still yielded.

**Lua needed no fix at all, which was worth confirming rather than assuming.** `mlua`'s default
`Lua::new()` gives the emitted Lua's own `io.lines()` unrestricted, direct access to the real
process stdin -- verified with a standalone program before writing a line of the real emitter,
since the alternative (a sandboxed or intercepted stdin) would have meant a different design for
that one backend. It did not, and Lua's collect helper is three lines because of it.

The one gap left open rather than fixed: a multi-byte UTF-8 character split exactly across the
JavaScript backend's 65536-byte chunk boundary decodes incorrectly, since neither the emitted
code's own chunk-by-chunk decoding nor the site's browser shim carries a partial character over
to the next chunk. Every corpus fixture is small enough that no chunk boundary is ever reached,
so this is latent rather than observed, and is noted here so it does not have to be rediscovered.

This is the sixth instance of
[the backend language has rules the checker does not](the-backend-language-has-rules-the-checker-does-not.md),
following
[a statically typed target asks for the types the checker already has](a-statically-typed-target-asks-for-the-types-the-checker-already-has.md).
Unlike the prior five, none of these were found by a program failing to compile or run --
Go's would have silently produced the wrong bytes on real CRLF input while every test using only
`\n` stayed green, which is the same shape as
[backends can agree and still be wrong](backends-can-agree-and-still-be-wrong.md): agreement is
evidence only on the axis actually being tested, and nothing before this exercised a carriage
return at all.

See [streaming input is pull, verified against jq itself](streaming-input-is-pull-verified-against-jq-itself.md)
for the model these facts were checked against.
