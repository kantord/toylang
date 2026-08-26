---
type: Lesson
calendar:
  - 2026-08-26
title: Juxtaposition is unsafe at any undelimited boundary
description: Adding parenless function application broke a program with no application in it, because two unrelated expressions had always sat adjacent at one spot the grammar never bothered to delimit.
tags:
  - parsing
  - syntax
  - design
timestamp: 2026-08-26T00:00:00Z
---

`fn f(x: Int) -> Int = x` followed by `f(1)` stopped compiling the moment bare application
(`f x` meaning `f(x)`) was added, with no bare application anywhere in the source. The error was
`expected an expression, found end of program`: the whole program body had been consumed as the
argument to `x`, the last token of `f`'s own definition.

The file grammar is `(fn ... | type ...)* body`. Every other place an expression is parsed fresh
-- a pipe's right side, inside `(...)`/`[...]`/`{...}`, the program's own `body` -- is either
preceded by a token that anchors it (`|`, `(`, `[`, `{`) or followed by one that bounds it (`)`,
`]`, `}`, or `Eof` for `body`). A definition's own body is the one exception: nothing marks where
it ends. The loop that reads definitions just keeps going while it sees `fn`/`type` and falls
through to `body` when it does not, so a definition's last token sits directly next to whatever
comes next with no separator at all -- another definition (safe, because `fn`/`type` cannot start
a bare argument) or the program's own body (unsafe, because an identifier can).

The fix is a parser flag, off for exactly the undelimited top-level chain of a definition's body
and restored the instant a real delimiter is entered (parens, brackets, braces), since a closing
token bounds those regardless of what sits outside them. It has to be threaded through every
place `expr` is called fresh from inside such a delimiter -- five call sites in this parser --
because forgetting one silently reopens the hazard for that one construct rather than failing
loudly.

What is still open: this is a manual invariant with no structural enforcement, the same shape as
[one invariant, three independent construction sites](one-invariant-three-independent-construction-sites.md)
-- nothing stops a future grammar addition from introducing a new undelimited adjacency, or a new
bracketed construct from forgetting to re-open the flag. The general lesson travels beyond this
parser: whitespace-insensitive juxtaposition syntax is only as safe as the delimiter discipline of
every boundary it can reach, and a grammar that has gotten away without delimiters somewhere (here,
between definitions and the body) has been relying on no construct ever needing to look past that
boundary -- which stops being true the moment one does.

The fix itself was later ported onto `winnow` unchanged in shape, which is its own finding: see
[winnow replaced the tokenizer, not the grammar](winnow-replaced-the-tokenizer-not-the-grammar.md).
