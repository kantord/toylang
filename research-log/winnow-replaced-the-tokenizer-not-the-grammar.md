---
type: Lesson
calendar:
  - 2026-08-26
title: winnow replaced the tokenizer, not the grammar
description: Porting the hand-rolled lexer and parser onto winnow kept every dispatch decision and error message exactly as written, because the library's real value here was position tracking and character-level primitives, not its combinators.
tags:
  - parsing
  - winnow
  - design
timestamp: 2026-08-26T00:00:00Z
---

The motivating case for adopting `winnow` was the bug in
[juxtaposition is unsafe at any undelimited boundary](juxtaposition-is-unsafe-at-any-undelimited-boundary.md):
a hand-threaded boolean flag enforcing a parser invariant, forgettable at any new call site. The
expectation going in was that a combinator library's declarative style -- `alt`, `cut_err`,
`separated`, and winnow 1.0's own Pratt-parsing `expression()` combinator -- would replace the
hand-written recursive-descent control flow wholesale.

That is not what happened, and the reason is worth recording: **every existing dispatch decision
in the old parser was already unambiguous, chosen by looking at exactly one token, with an exact
error message for exactly why it failed.** `alt`'s job is choosing between branches that might
each fail and need to be tried in order; toylang's grammar never has that shape; the old parser
always already knew which branch to take from the next token, the same way `read_tok`'s match on
the next character does now. Replacing a deterministic `match` with a combinator that exists to
handle backtracking would have added a mechanism the grammar does not need, and risked losing the
exact error text (`"expected {want}, found {other}"`, span reported by `.start` only, per byte)
that 105 tests assert on literally.

So the port kept the entire recursive-descent structure -- `expr`, `root`, `operand`, `unary`,
`postfix`, `atom`, `def`, `alias`, every hand-matched dispatch -- and used winnow only for what it
is actually good at under the hood: `LocatingSlice<&str>` for byte-offset spans without manual
index bookkeeping, `Stream::next_token`/`peek_token` for character-level cursor movement with
UTF-8 handled for free, and three `take_while` calls for the three genuinely uniform character
runs (whitespace, digits, identifier tails). The capability-token fix for the undelimited-boundary
bug -- a `bare_ok` flag restored to `true` by a `delimited` helper wrapping every real bracket --
ported over unchanged in shape; the research beforehand had already confirmed this is a plain
Rust-idiom question, not a winnow feature (see
[juxtaposition is unsafe at any undelimited boundary](juxtaposition-is-unsafe-at-any-undelimited-boundary.md)).
The library's own `ParserError` trait was implemented directly for this crate's existing `Error`
type, so there is no translation layer between "a winnow combinator failed" and "toylang reports
an error" -- one error type, used exactly as it was before winnow existed.

Result: every one of 105 tests, including every snapshot asserting an error message's exact text,
passed unchanged -- no snapshot needed regenerating. That is the actual evidence the port was
behavior-preserving, not an assumption: the previous lexer and parser were a genuinely separate
pass (`lex()` producing a `Vec<Token>`, then `parse()` consuming it), and the new one tokenizes
on demand, one token at a time, as the recursive descent asks for it.

That merge changed one real thing, confirmed deliberately rather than caught by accident: with a
separate up-front lexing pass, any lexical error anywhere in the file -- a bad string escape, an
out-of-range integer -- was always reported before any parse error, regardless of position, because
the whole file had to tokenize successfully before parsing began at all. On-demand tokenizing
drops that guarantee: whichever error the recursive descent reaches first now wins, lexical or
not. `fn f(x: Int` (missing punctuation) followed by `"bad \q escape"` on the next line now
reports the escape error, not the missing-paren one, because `def`'s search for the closing `)`
has to tokenize the next thing to compare against it, and that next thing is the bad string. No
existing test depended on the old ordering; this is now the actual, disclosed rule rather than
an accident of implementation.

What is still open: winnow 1.0's `expression()` Pratt combinator was never tried against the
existing `min_power`-threaded precedence table, which also drives the pipe and ternary and the
root/operand split for bare application -- it is not clear that combinator's own precedence
tracking composes with that reuse at all, and finding out would have meant risking the exact
parity this port achieved for a mechanism the current grammar does not need today.
