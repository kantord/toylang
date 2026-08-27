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

A second instance surfaced while writing enum corpus cases, and the flag does not cover it:
`fn g(s: Int) -> Int = 1` followed by a program body of `[g(1), g(2)]` fails with
`expected ']', found ','`, because postfix indexing reaches across the same boundary and reads
the body's Vec literal as `1[g(1), ...]`. Bare application was suspended at that boundary;
`[` was not, and suspending it the same way would break legitimate indexing inside a
definition's body (`v[0]!`). Nothing in the corpus had ever put a `[`-literal body directly
after a definition, which is how it stayed invisible for as long as bare application's version
did. And a third: the parenless record-argument form (`f {..}` as a call) reaches across the
same spot whenever a definition's body *ends* in an identifier and the program body starts
with `{` -- `... -> r * r` followed by `{a: ...}` reads the record as an argument to `r`. That
form was justified as "was a syntax error before, so nothing is taken away," which is true
inside any delimiter and false at exactly this boundary. Until the boundary gets a real
delimiter (or the grammar goes newline-sensitive), a program body cannot start with `[` after
a definition, nor with `{` when the definition ends in an identifier -- worked around in the
corpus by shaping bodies to avoid both.

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
